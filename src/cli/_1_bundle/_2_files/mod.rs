use super::*;

pub(super) fn new_dir(path: &Path) -> Result<(), Error> {
    fs::create_dir(path).map_err(|e| {
        error(
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                "INSTALL_EXISTS"
            } else {
                "INSTALL_IO"
            },
            e,
        )
    })?;
    set_mode(path, 0o700)
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| error("INSTALL_IO", e))?;
    file.write_all(bytes).map_err(|e| error("INSTALL_IO", e))?;
    file.sync_all().map_err(|e| error("INSTALL_IO", e))
}

pub(super) fn copy(
    root: &Path,
    source: &Path,
    relative: &str,
    executable: bool,
    expected: &Digest,
) -> Result<Entry, Error> {
    super::relative(relative)?;
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| error("INSTALL_IO", e))?;
    }
    let mut input = fs::File::open(source).map_err(|e| error("INSTALL_IO", e))?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| error("INSTALL_IO", e))?;
    let bytes = std::io::copy(&mut input, &mut output).map_err(|e| error("INSTALL_IO", e))?;
    output.sync_all().map_err(|e| error("INSTALL_IO", e))?;
    set_mode(&path, if executable { 0o500 } else { 0o400 })?;
    let sha256 = Digest::new(file_sha256(&path).map_err(|e| error("INSTALL_IO", e))?)?;
    if &sha256 != expected {
        return Err(error("BUNDLE_INTEGRITY", "file changed while copying"));
    }
    Ok(Entry {
        path: relative.into(),
        sha256,
        bytes,
        executable,
    })
}
