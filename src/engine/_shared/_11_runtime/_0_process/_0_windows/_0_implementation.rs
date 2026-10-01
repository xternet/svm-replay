use super::*;

pub(in super::super::super) fn execute(
    worker: &WorkerSpec,
    owner: Option<&ProcessOwner>,
    cwd: &Path,
    input: &Path,
    output: &Path,
    flags: &[String],
    output_files: &[PathBuf],
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    diagnostics: Option<&SyncSender<Vec<u8>>>,
) -> Result<Execution, WorkerError> {
    let system_root = std::env::var_os("SystemRoot").ok_or_else(|| {
        WorkerError::new(
            WorkerErrorCode::Io,
            "Windows worker networking requires the SystemRoot environment variable",
        )
    })?;
    if let Some(owner) = owner {
        let current = std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .map_err(|e| WorkerError::io("resolve Windows job owner", e))?;
        let requested = owner
            .executable
            .canonicalize()
            .map_err(|e| WorkerError::io("resolve requested job owner", e))?;
        if current != requested {
            return Err(WorkerError::new(
                WorkerErrorCode::UnsupportedPlatform,
                "Windows jobs are owned by the calling executable; external owners are unsupported",
            ));
        }
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new().name("svm-windows-worker".into()).spawn_scoped(scope, || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()
                .map_err(|e| WorkerError::io("create native I/O runtime", e))?;
            runtime.block_on(async {
                let mut command = CommandWrap::with_new(&worker.executable, |command| {
                    command.arg(input).arg(output).args(flags).current_dir(cwd).env_clear()
                        .env("SystemRoot", &system_root)
                        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
                });
                command.wrap(KillOnDrop).wrap(JobObject);
                budget.check()?;
                let mut child = command.spawn().map_err(|e| WorkerError::io("spawn pinned Windows worker", e))?;
                // These handles exist because this command has just spawned with piped I/O.
                let pid = child.id().expect("new worker PID");
                let mut out = child.stdout().take().expect("piped worker stdout");
                let mut err = child.stderr().take().expect("piped worker stderr");
                let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
                let (mut out_done, mut err_done) = (false, false);
                let (mut out_chunk, mut err_chunk) = ([0u8; 16384], [0u8; 16384]);
                let mut result = async {
                    loop {
                        budget.check()?;
                        super::super::super::check_output(output, limits.max_output_bytes)?;
                        for path in output_files {
                            super::super::super::check_output(path, limits.max_output_bytes)?;
                        }
                        // Poll only the primary child here. Job completion is consumed once,
                        // during cleanup, so its completion notification cannot be lost.
                        if let Some(status) = child.inner_mut().try_wait().map_err(|e| WorkerError::io("poll Windows worker", e))? {
                            if !status.success() {
                                return Err(WorkerError::new(WorkerErrorCode::WorkerExit, format!("worker exited with {status}")));
                            }
                            return Ok(());
                        }
                        tokio::select! {
                            count = out.read(&mut out_chunk), if !out_done => {
                                let count = count.map_err(|e| WorkerError::io("read worker stdout", e))?;
                                out_done = count == 0;
                                append(&mut stdout, stderr.len(), &out_chunk[..count], limits.max_diagnostic_bytes)?;
                            }
                            count = err.read(&mut err_chunk), if !err_done => {
                                let count = count.map_err(|e| WorkerError::io("read worker stderr", e))?;
                                err_done = count == 0;
                                append(&mut stderr, stdout.len(), &err_chunk[..count], limits.max_diagnostic_bytes)?;
                                send(diagnostics, &err_chunk[..count])?;
                            }
                            _ = tokio::time::sleep(Duration::from_millis(5)) => {}
                        }
                    }
                }.await;
                // Terminate every remaining process even after a successful primary exit.
                // Closing the non-inherited job handle also kills the job if the host dies.
                let cleanup = async {
                    child.start_kill().map_err(|e| WorkerError::io("terminate Windows job", e))?;
                    tokio::time::timeout(limits.cleanup_grace, child.wait()).await
                        .map_err(|_| WorkerError::new(WorkerErrorCode::Cleanup, "Windows job cleanup deadline exceeded"))?
                        .map_err(|e| WorkerError::io("wait for Windows job", e))?;
                    while !out_done || !err_done {
                        tokio::select! {
                            count = out.read(&mut out_chunk), if !out_done => {
                                let count = count.map_err(|e| WorkerError::io("drain worker stdout", e))?;
                                out_done = count == 0;
                                append(&mut stdout, stderr.len(), &out_chunk[..count], limits.max_diagnostic_bytes)?;
                            }
                            count = err.read(&mut err_chunk), if !err_done => {
                                let count = count.map_err(|e| WorkerError::io("drain worker stderr", e))?;
                                err_done = count == 0;
                                append(&mut stderr, stdout.len(), &err_chunk[..count], limits.max_diagnostic_bytes)?;
                                send(diagnostics, &err_chunk[..count])?;
                            }
                        }
                    }
                    super::super::super::check_output(output, limits.max_output_bytes)?;
                    for path in output_files { super::super::super::check_output(path, limits.max_output_bytes)?; }
                    Ok::<(), WorkerError>(())
                }.await;
                if let Err(mut error) = cleanup {
                    if let Err(primary) = &result { error.message.push_str(&format!("; preceding failure: {primary}")); }
                    result = Err(error);
                }
                match result {
                    Ok(()) => Ok(Execution { pid, stdout, stderr }),
                    Err(mut error) => { error.pid = Some(pid); error.stdout = stdout; error.stderr = stderr; Err(error) }
                }
            })
        }).map_err(|e| WorkerError::io("spawn native I/O thread", e))?.join()
            .map_err(|_| WorkerError::new(WorkerErrorCode::Io, "native I/O thread panicked"))?
    })
}

pub(super) fn append(
    target: &mut Vec<u8>,
    other: usize,
    bytes: &[u8],
    limit: usize,
) -> Result<(), WorkerError> {
    if target
        .len()
        .checked_add(other)
        .and_then(|n| n.checked_add(bytes.len()))
        .is_none_or(|n| n > limit)
    {
        return Err(WorkerError::new(
            WorkerErrorCode::DiagnosticLimit,
            "combined worker diagnostics exceed limit",
        ));
    }
    target.extend_from_slice(bytes);
    Ok(())
}
pub(super) fn send(sender: Option<&SyncSender<Vec<u8>>>, bytes: &[u8]) -> Result<(), WorkerError> {
    if let Some(sender) = sender.filter(|_| !bytes.is_empty()) {
        sender.try_send(bytes.to_vec()).map_err(|e| {
            WorkerError::new(
                WorkerErrorCode::DiagnosticLimit,
                format!("live diagnostic consumer cannot accept bounded chunk: {e}"),
            )
        })?;
    }
    Ok(())
}
