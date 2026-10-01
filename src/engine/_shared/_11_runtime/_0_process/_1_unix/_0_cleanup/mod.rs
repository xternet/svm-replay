use super::*;
pub(super) fn cleanup(
    child: &mut std::process::Child,
    owner: Option<&ProcessOwner>,
    pid: u32,
    limits: &WorkerLimits,
    drain: &mut impl FnMut(),
) -> Result<(), WorkerError> {
    let cleanup = (|| {
        let running = child
            .try_wait()
            .map_err(|error| WorkerError::io("poll worker before cleanup", error))?
            .is_none();
        if running {
            signal(
                if owner.is_some() {
                    pid as i32
                } else {
                    -(pid as i32)
                },
                if owner.is_some() {
                    libc::SIGTERM
                } else {
                    libc::SIGKILL
                },
            )?;
            let deadline = Instant::now() + limits.cleanup_grace;
            while child
                .try_wait()
                .map_err(|error| WorkerError::io("await worker cleanup", error))?
                .is_none()
            {
                drain();
                if Instant::now() >= deadline {
                    signal(-(pid as i32), libc::SIGKILL)?;
                    child
                        .wait()
                        .map_err(|error| WorkerError::io("reap force-killed worker", error))?;
                    return Err(WorkerError::new(WorkerErrorCode::Cleanup, "cleanup grace exceeded; immediate child reaped but owner descendant cleanup is unconfirmed"));
                }
                thread::sleep(Duration::from_millis(5));
            }
        }
        let status = child
            .wait()
            .map_err(|error| WorkerError::io("reap worker", error))?;
        if running
            && owner.is_some()
            && !status.success()
            && status.code() != Some(128 + libc::SIGTERM)
        {
            return Err(WorkerError::new(
                WorkerErrorCode::Cleanup,
                format!("owner termination did not confirm cleanup: {status}"),
            ));
        }
        if group_exists(pid)? {
            signal(-(pid as i32), libc::SIGKILL)?;
            return Err(WorkerError::new(WorkerErrorCode::Cleanup, "worker exited leaving process-group descendants; group killed, reaping unconfirmed"));
        }
        Ok(())
    })();
    cleanup
}
