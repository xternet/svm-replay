use super::*;
pub(super) fn spawn(
    worker: &WorkerSpec,
    owner: Option<&ProcessOwner>,
    cwd: &Path,
    input: &Path,
    output: &Path,
    flags: &[String],
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
) -> Result<std::process::Child, WorkerError> {
    let executable = owner.map_or(worker.executable.as_path(), |owner| {
        owner.executable.as_path()
    });
    let mut command = Command::new(executable);
    if owner.is_some() {
        command
            .arg("__worker-owner")
            .arg("--")
            .arg(&worker.executable);
    }
    command
        .arg(input)
        .arg(output)
        .args(flags)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let parent_pid = unsafe { libc::getpid() };
    let file_limit = limits.max_output_bytes as libc::rlim_t;
    let death_signal = if owner.is_some() {
        libc::SIGTERM
    } else {
        libc::SIGKILL
    };
    // These async-signal-safe controls run only in the forked child. The SDK's
    // embedding process keeps its own signal handlers, environment and subreaper.
    unsafe {
        command.pre_exec(move || {
            let bound = libc::rlimit {
                rlim_cur: file_limit,
                rlim_max: file_limit,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &bound) == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            #[cfg(target_os = "linux")]
            if libc::prctl(libc::PR_SET_PDEATHSIG, death_signal) == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != parent_pid {
                return Err(io::Error::from_raw_os_error(libc::ECHILD));
            }
            Ok(())
        });
    }
    budget.check()?;
    let child = command
        .spawn()
        .map_err(|error| WorkerError::io("spawn pinned worker", error))?;
    Ok(child)
}
