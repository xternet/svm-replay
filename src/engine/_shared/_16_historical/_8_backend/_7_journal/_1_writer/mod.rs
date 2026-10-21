use std::{
    fs::{File, OpenOptions, TryLockError},
    path::Path,
};
use svm_replay_protocol::Error;

/// Lock the journal's actual inode (including path aliases), before reading or writing it.
/// The returned descriptor owns the lease until the backend closes or exits.
pub(super) fn open(path: &Path) -> Result<(File, bool), Error> {
    let io = |error: std::io::Error| Error::new("JOURNAL_IO", error.to_string());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io)?;
    }
    let (file, created) = match OpenOptions::new()
        .create_new(true)
        .read(true)
        .append(true)
        .open(path)
    {
        Ok(file) => (file, true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (
            OpenOptions::new()
                .read(true)
                .append(true)
                .open(path)
                .map_err(io)?,
            false,
        ),
        Err(error) => return Err(io(error)),
    };
    file.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => Error::new(
            "JOURNAL_IN_USE",
            "another historical session owns this journal",
        ),
        TryLockError::Error(error) => io(error),
    })?;
    Ok((file, created))
}
