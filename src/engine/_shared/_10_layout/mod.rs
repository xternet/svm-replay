//! Application data is separate from installation and source code.
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
use svm_replay_protocol::Error;

pub fn validate(root: &Path) -> Result<(), Error> {
    if !root.is_absolute() {
        return Err(Error::new(
            "DATA_DIRECTORY",
            "resolve the data directory explicitly",
        ));
    }
    for name in ["index.sqlite", "blobs", "jobs"] {
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Ok(_) => return Err(Error::new(
                "LEGACY_DATA_LAYOUT",
                "legacy data root detected; use a fresh --data-dir; existing data is not modified",
            )
            .with_details(serde_json::json!({"path":path}))),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(Error::new(
                    "DATA_DIRECTORY",
                    format!("{}: {error}", path.display()),
                ))
            }
        }
    }
    Ok(())
}

pub fn cache_dir(root: &Path) -> Result<PathBuf, Error> {
    validate(root)?;
    Ok(root.join("cache"))
}

pub fn runs_dir(root: &Path) -> Result<PathBuf, Error> {
    validate(root)?;
    Ok(root.join("runs"))
}

#[path = "tests.rs"]
#[cfg(test)]
mod tests;
