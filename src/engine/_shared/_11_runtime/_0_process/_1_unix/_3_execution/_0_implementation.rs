use super::*;

pub(crate) fn execute(
    worker: &WorkerSpec,
    owner: Option<&ProcessOwner>,
    cwd: &Path,
    input: &Path,
    output: &Path,
    flags: &[String],
    output_files: &[PathBuf],
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    diagnostics: Option<&std::sync::mpsc::SyncSender<Vec<u8>>>,
) -> Result<Execution, WorkerError> {
    let mut child = spawn(worker, owner, cwd, input, output, flags, limits, budget)?;
    let pid = child.id();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut sent_stderr = 0;
    let mut send_diagnostics = |bytes: &[u8]| -> Result<(), WorkerError> {
        if let Some(sender) = diagnostics {
            if bytes.len() > sent_stderr {
                sender
                    .try_send(bytes[sent_stderr..].to_vec())
                    .map_err(|error| {
                        WorkerError::new(
                            WorkerErrorCode::DiagnosticLimit,
                            format!(
                                "live diagnostic consumer cannot accept bounded chunk: {error}"
                            ),
                        )
                    })?;
                sent_stderr = bytes.len();
            }
        }
        Ok(())
    };
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let mut nonblocking = false;
    let mut result = (|| {
        let out = out_pipe.as_mut().ok_or_else(|| {
            WorkerError::new(WorkerErrorCode::Io, "spawned worker has no stdout pipe")
        })?;
        let err = err_pipe.as_mut().ok_or_else(|| {
            WorkerError::new(WorkerErrorCode::Io, "spawned worker has no stderr pipe")
        })?;
        for descriptor in [out.as_raw_fd(), err.as_raw_fd()] {
            let current = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
            if current == -1
                || unsafe { libc::fcntl(descriptor, libc::F_SETFL, current | libc::O_NONBLOCK) }
                    == -1
            {
                return Err(WorkerError::io(
                    "set diagnostic pipe nonblocking",
                    io::Error::last_os_error(),
                ));
            }
        }
        nonblocking = true;
        loop {
            budget.check()?;
            collect(out, &mut stdout, stderr.len(), limits.max_diagnostic_bytes)?;
            collect(err, &mut stderr, stdout.len(), limits.max_diagnostic_bytes)?;
            send_diagnostics(&stderr)?;
            super::super::super::super::check_output(output, limits.max_output_bytes)?;
            for path in output_files {
                super::super::super::super::check_output(path, limits.max_output_bytes)?;
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|error| WorkerError::io("poll worker", error))?
            {
                collect(out, &mut stdout, stderr.len(), limits.max_diagnostic_bytes)?;
                collect(err, &mut stderr, stdout.len(), limits.max_diagnostic_bytes)?;
                send_diagnostics(&stderr)?;
                super::super::super::super::check_output(output, limits.max_output_bytes)?;
                for path in output_files {
                    super::super::super::super::check_output(path, limits.max_output_bytes)?;
                }
                if status.signal() == Some(libc::SIGXFSZ)
                    || (owner.is_some() && status.code() == Some(128 + libc::SIGXFSZ))
                {
                    return Err(WorkerError::new(
                        WorkerErrorCode::OutputLimit,
                        format!(
                            "worker exceeded per-file byte limit {}",
                            limits.max_output_bytes
                        ),
                    ));
                }
                if !status.success() {
                    return Err(WorkerError::new(
                        WorkerErrorCode::WorkerExit,
                        format!("worker exited with {status}"),
                    ));
                }
                return Ok(());
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    let mut drain = || {
        if nonblocking {
            let observed = (|| {
                if let Some(out) = out_pipe.as_mut() {
                    collect(out, &mut stdout, stderr.len(), limits.max_diagnostic_bytes)?;
                }
                if let Some(err) = err_pipe.as_mut() {
                    collect(err, &mut stderr, stdout.len(), limits.max_diagnostic_bytes)?;
                }
                Ok::<(), WorkerError>(())
            })();
            if let Err(error) = observed {
                match &mut result {
                    Ok(()) => result = Err(error),
                    Err(primary) => primary
                        .message
                        .push_str(&format!("; cleanup diagnostics: {error}")),
                }
            }
        }
    };
    // Always wait for the immediate child, including every setup/read/monitor error.
    // Owners receive TERM first so they can kill and reap their own descendants.
    let cleanup = cleanup(&mut child, owner, pid, limits, &mut drain);
    drain();
    drop(drain);
    // A cleanup syscall failure must not bypass the final direct-child kill/wait.
    let cleanup = match cleanup {
        Ok(()) => Ok(()),
        Err(mut error) => {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    if let Err(kill_error) = child.kill() {
                        error
                            .message
                            .push_str(&format!("; final immediate-child kill: {kill_error}"));
                    }
                    if let Err(wait_error) = child.wait() {
                        error
                            .message
                            .push_str(&format!("; final immediate-child wait: {wait_error}"));
                    }
                }
                Err(wait_error) => {
                    error
                        .message
                        .push_str(&format!("; final child state: {wait_error}"));
                    if let Err(kill_error) = child.kill() {
                        error
                            .message
                            .push_str(&format!("; final immediate-child kill: {kill_error}"));
                    }
                    if let Err(wait_error) = child.wait() {
                        error
                            .message
                            .push_str(&format!("; final immediate-child wait: {wait_error}"));
                    }
                }
            }
            Err(error)
        }
    };
    let result = match (result, cleanup) {
        (result, Ok(())) => result,
        (Ok(()), Err(error)) => Err(error),
        (Err(primary), Err(mut cleanup)) => {
            cleanup.message = format!(
                "{}; preceding failure {:?}: {}",
                cleanup.message, primary.code, primary.message
            );
            Err(cleanup)
        }
    };
    match result {
        Ok(()) => Ok(Execution {
            pid,
            stdout,
            stderr,
        }),
        Err(mut error) => {
            error.pid = Some(pid);
            error.stdout = stdout;
            error.stderr = stderr;
            Err(error)
        }
    }
}

pub(in super::super) fn signal(target: i32, signal: i32) -> Result<(), WorkerError> {
    if unsafe { libc::kill(target, signal) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(WorkerError::new(
        WorkerErrorCode::Cleanup,
        format!("signal {signal} to {target}: {error}"),
    ))
}

pub(in super::super) fn group_exists(pid: u32) -> Result<bool, WorkerError> {
    if unsafe { libc::kill(-(pid as i32), 0) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(false);
    }
    Err(WorkerError::new(
        WorkerErrorCode::Cleanup,
        format!("inspect process group {pid}: {error}"),
    ))
}
