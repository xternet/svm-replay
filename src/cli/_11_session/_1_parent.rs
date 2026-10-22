use svm_replay_engine::shared::runtime::CancellationToken;
use svm_replay_protocol::Error;

pub(super) fn bind(expected: Option<u32>, _cancellation: CancellationToken) -> Result<(), Error> {
    let Some(expected) = expected else {
        return Ok(());
    };
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if expected <= 1 || unsafe { libc::getppid() } as u32 != expected {
        return Err(Error::new(
            "SESSION_PARENT",
            "SDK parent PID differs from the actual process parent",
        ));
    }
    #[cfg(target_os = "linux")]
    unsafe {
        if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
            return Err(Error::new(
                "SESSION_PARENT",
                format!("bind parent lifetime: {}", std::io::Error::last_os_error()),
            ));
        }
        if libc::getppid() as u32 != expected {
            return Err(Error::new(
                "SESSION_PARENT",
                "SDK parent exited during session startup",
            ));
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        // Installed before input preparation. Like the macOS worker owner, monitor
        // the actual parent; cancellation also interrupts preparation and joins
        // the native session instead of relying on stdin reaching EOF after ready.
        std::thread::Builder::new()
            .name("session-parent".into())
            .spawn(move || {
                while !_cancellation.is_cancelled() {
                    if unsafe { libc::getppid() } as u32 != expected {
                        _cancellation.cancel();
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            })
            .map_err(|error| Error::new("SESSION_PARENT", format!("watch SDK parent: {error}")))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(Error::new(
            "UNSUPPORTED_PLATFORM",
            "SDK parent lifetime binding requires Linux or macOS",
        ))
    }
}
