use super::*;

pub fn run(args: &[OsString]) -> Result<i32, String> {
    use std::{
        io,
        os::unix::process::CommandExt,
        process::{Command, Stdio},
        sync::atomic::Ordering,
        thread,
        time::{Duration, Instant},
    };
    let executable = args.first().ok_or("worker owner requires an executable")?;
    if !std::path::Path::new(executable).is_absolute() {
        return Err("worker owner requires an absolute executable path".into());
    }
    let initial_parent = unsafe { libc::getppid() };
    // The root dispatch enters this mode before any application threads start.
    // All process-wide state below belongs to this short-lived helper only.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = terminate as *const () as usize;
        if libc::sigemptyset(&mut action.sa_mask) != 0 {
            return Err(format!("owner sigemptyset: {}", io::Error::last_os_error()));
        }
        for signal in [libc::SIGTERM, libc::SIGINT] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(format!(
                    "owner sigaction {signal}: {}",
                    io::Error::last_os_error()
                ));
            }
        }
        if libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) != 0 {
            return Err(format!(
                "owner subreaper setup: {}",
                io::Error::last_os_error()
            ));
        }
        if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
            return Err(format!(
                "owner parent-death setup: {}",
                io::Error::last_os_error()
            ));
        }
    }
    if unsafe { libc::getppid() } != initial_parent || STOP.load(Ordering::Acquire) {
        return Ok(128 + libc::SIGTERM);
    }
    let owner_pid = unsafe { libc::getpid() };
    let mut command = Command::new(executable);
    command
        .args(&args[1..])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != owner_pid {
                return Err(io::Error::from_raw_os_error(libc::ECHILD));
            }
            Ok(())
        });
    }
    let child = command
        .spawn()
        .map_err(|error| format!("owner spawn {}: {error}", executable.to_string_lossy()))?;
    let worker_pid = child.id() as libc::pid_t;
    // Raw waitpid(-1) below must reap both the primary and adopted descendants.
    // Dropping Child only closes its empty handles; it does not kill or wait.
    drop(child);
    let mut worker_status = None;
    let mut cleanup_started = None;
    let mut cancelled = false;
    let mut failures = Vec::new();
    loop {
        if STOP.load(Ordering::Acquire) || unsafe { libc::getppid() } != initial_parent {
            cancelled = true;
            cleanup_started.get_or_insert_with(Instant::now);
        }
        let no_children = loop {
            let mut status = 0;
            let reaped = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
            if reaped > 0 {
                if reaped == worker_pid {
                    worker_status = Some(status);
                    cleanup_started.get_or_insert_with(Instant::now);
                }
                continue;
            }
            if reaped == 0 {
                break false;
            }
            let error = io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::ECHILD) => break true,
                Some(libc::EINTR) => continue,
                _ => {
                    failures.push(format!("owner waitpid: {error}"));
                    cleanup_started.get_or_insert_with(Instant::now);
                    break false;
                }
            }
        };
        if no_children {
            if !failures.is_empty() {
                return Err(failures.join("; "));
            }
            if cancelled {
                return Ok(128 + libc::SIGTERM);
            }
            let status = worker_status.ok_or("owner lost primary child status before reaping")?;
            if libc::WIFEXITED(status) {
                return Ok(libc::WEXITSTATUS(status));
            }
            if libc::WIFSIGNALED(status) {
                return Ok(128 + libc::WTERMSIG(status));
            }
            return Err(format!(
                "owner primary returned unexpected wait status {status}"
            ));
        }
        if let Some(started) = cleanup_started {
            match direct_children(owner_pid) {
                Ok(children) => {
                    for pid in children {
                        if let Err(error) = kill_child(pid) {
                            failures.push(error);
                        }
                    }
                }
                Err(error) => {
                    failures.push(error);
                    // The primary PID is still authoritative if enumeration fails.
                    if worker_status.is_none() {
                        if let Err(error) = kill_child(worker_pid) {
                            failures.push(error);
                        }
                    }
                }
            }
            // This is an explicit cleanup failure, never an exit-zero receipt.
            // Kernel-uninterruptible children cannot be promised a finite reap.
            if started.elapsed() >= Duration::from_millis(1500) {
                let remaining = direct_children(owner_pid);
                return Err(format!("owner cleanup deadline exceeded; remaining children {remaining:?}; failures {failures:?}"));
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}

pub(super) static STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) extern "C" fn terminate(_: libc::c_int) {
    STOP.store(true, std::sync::atomic::Ordering::Release);
}

pub(super) fn direct_children(owner_pid: libc::pid_t) -> Result<Vec<libc::pid_t>, String> {
    use std::io::Read;
    let path = format!("/proc/self/task/{owner_pid}/children");
    let file = std::fs::File::open(&path).map_err(|error| format!("owner open {path}: {error}"))?;
    let mut text = String::new();
    file.take(1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|error| format!("owner read {path}: {error}"))?;
    if text.len() > 1024 * 1024 {
        return Err("owner child PID list exceeds 1 MiB".into());
    }
    text.split_whitespace()
        .map(|value| {
            let pid = value
                .parse::<libc::pid_t>()
                .map_err(|error| format!("owner invalid child PID {value}: {error}"))?;
            if pid <= 0 {
                return Err(format!("owner invalid child PID {pid}"));
            }
            Ok(pid)
        })
        .collect()
}

pub(super) fn kill_child(pid: libc::pid_t) -> Result<(), String> {
    if unsafe { libc::kill(pid, libc::SIGKILL) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(format!("owner SIGKILL child {pid}: {error}"))
}
