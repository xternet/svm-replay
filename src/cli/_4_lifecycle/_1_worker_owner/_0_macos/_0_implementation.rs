use super::*;

pub(super) static STOP: AtomicBool = AtomicBool::new(false);
pub(super) extern "C" fn terminate(_: libc::c_int) {
    STOP.store(true, Ordering::Release);
}

pub fn run(args: &[OsString]) -> Result<i32, String> {
    let executable = args.first().ok_or("owner requires executable")?;
    if !std::path::Path::new(executable).is_absolute() {
        return Err("owner requires absolute executable".into());
    }
    let parent = unsafe { libc::getppid() };
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = terminate as *const () as usize;
        if libc::sigemptyset(&mut action.sa_mask) != 0 {
            return Err(format!("owner sigemptyset: {}", io::Error::last_os_error()));
        }
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(format!("owner sigaction: {}", io::Error::last_os_error()));
            }
        }
    }
    let owner = unsafe { libc::getpid() };
    let mut command = Command::new(executable);
    command
        .args(&args[1..])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != owner {
                return Err(io::Error::from_raw_os_error(libc::ECHILD));
            }
            Ok(())
        });
    }
    let mut child = command.spawn().map_err(|e| format!("owner spawn: {e}"))?;
    let pid = child.id() as i32;
    let mut cancelled = false;
    let result = loop {
        if STOP.load(Ordering::Acquire) || unsafe { libc::getppid() } != parent {
            cancelled = true;
            break Ok(None);
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(Some(status)),
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(error) => break Err(format!("owner poll: {error}")),
        }
    };
    // The group is separate from this owner, so killing it cannot kill the
    // cleanup observer before it has waited for the primary child.
    let killed = unsafe { libc::kill(-pid, libc::SIGKILL) };
    let kill_error = io::Error::last_os_error();
    if killed != 0 && kill_error.raw_os_error() != Some(libc::ESRCH) {
        if let Err(error) = child.kill() {
            eprintln!("owner emergency child kill: {error}");
        }
        if let Err(error) = child.wait() {
            eprintln!("owner emergency child wait: {error}");
        }
        return Err(format!("owner group kill: {kill_error}"));
    }
    let status = child.wait().map_err(|e| format!("owner reap: {e}"))?;
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut reported_pending = false;
    loop {
        if unsafe { libc::kill(-pid, 0) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                break;
            }
            // Darwin killpg1 excludes zombies, then returns EPERM for an
            // existing group with no signalable members. Await ESRCH; EPERM
            // never establishes successful cleanup and never triggers escalation.
            if error.raw_os_error() != Some(libc::EPERM) {
                return Err(format!("owner inspect group: {error}"));
            }
            if !reported_pending {
                eprintln!("owner group {pid} pending reap or inaccessible: {error}; awaiting disappearance");
                reported_pending = true;
            }
        }
        if Instant::now() >= deadline {
            return Err("owner process group still exists after cleanup".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
    result?;
    if cancelled {
        return Ok(128 + libc::SIGTERM);
    }
    status
        .code()
        .or_else(|| status.signal().map(|s| 128 + s))
        .ok_or_else(|| "owner lost child exit status".into())
}
