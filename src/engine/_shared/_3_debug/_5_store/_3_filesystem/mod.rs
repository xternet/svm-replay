use super::*;

pub(super) fn write_file(path: &Path, bytes: &[u8], executable: bool) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if executable { 0o500 } else { 0o400 });
    }
    let mut file = options
        .open(path)
        .map_err(|e| error("SESSION_IO", format!("create {}: {e}", path.display())))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| error("SESSION_IO", format!("write/sync {}: {e}", path.display())))
}

pub(super) fn sync_directory(directory: &Path) -> Result<(), Error> {
    let parent = directory
        .parent()
        .ok_or_else(|| error("SESSION_IO", "saved directory has no parent"))?;
    for path in [directory, parent] {
        File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(|e| {
                error(
                    "SESSION_IO",
                    format!("sync saved directory {}: {e}", path.display()),
                )
            })?;
    }
    Ok(())
}
