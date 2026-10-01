use std::path::PathBuf;
use svm_replay_protocol::Error;

pub fn data_directory(explicit: Option<PathBuf>) -> Result<PathBuf, Error> {
    let directory = match explicit {
        Some(directory) => directory,
        None => dirs::data_local_dir()
            .ok_or_else(|| {
                Error::new(
                    "DATA_DIRECTORY",
                    "no OS application-data directory; pass --data-dir",
                )
            })?
            .join("svm-replay"),
    };
    if directory.is_absolute() {
        Ok(directory)
    } else {
        Ok(std::env::current_dir()
            .map_err(|e| Error::new("DATA_DIRECTORY", e.to_string()))?
            .join(directory))
    }
}

#[path = "tests.rs"]
#[cfg(test)]
mod tests;
