use super::*;

pub fn write_json(path: &Path, value: &Value) -> Result<Digest, Error> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::new("OUTPUT_JSON", e.to_string()))?;
    write_bytes(path, &bytes)
}

pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<Digest, Error> {
    let mut temporary = tempfile::NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| Error::new("OUTPUT_PATH", "missing parent"))?,
    )
    .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?;
    temporary
        .persist_noclobber(path)
        .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?;
    #[cfg(unix)]
    File::open(
        path.parent()
            .ok_or_else(|| Error::new("OUTPUT_PATH", "missing parent"))?,
    )
    .and_then(|f| f.sync_all())
    .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?;
    Ok(Digest::of(bytes))
}

pub fn ensure_private_directory(path: &Path) -> Result<(), Error> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|e| Error::new("DATA_DIRECTORY", e.to_string()))?;
    let probe = tempfile::NamedTempFile::new_in(path)
        .map_err(|e| Error::new("DATA_DIRECTORY", e.to_string()))?;
    probe
        .close()
        .map_err(|e| Error::new("DATA_DIRECTORY", e.to_string()))?;
    Ok(())
}

/// Missing completion means incomplete, not proof the process is still alive or dead.
pub fn inspect_job(path: &Path) -> Result<Value, Error> {
    match crate::shared::runtime::read_bounded_file(&path.join("receipt.json"), 16 * 1024 * 1024) {
        Ok(bytes) => svm_replay_protocol::parse_json(&bytes),
        Err(error) => {
            if path
                .join("receipt.json")
                .try_exists()
                .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?
            {
                return Err(Error::new("OUTPUT_IO", error.to_string()));
            }
            let marker = path.join("started.json");
            if !marker
                .try_exists()
                .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?
            {
                return Err(Error::new(
                    "JOB_NOT_FOUND",
                    "no started marker or final receipt",
                ));
            }
            let bytes = crate::shared::runtime::read_bounded_file(&marker, 1024 * 1024)
                .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?;
            let started = svm_replay_protocol::parse_json(&bytes)?;
            Ok(
                serde_json::json!({"schema":"svm-replay-incomplete/v1","outcome":"INCOMPLETE","started":started,
                "meaning":"No final receipt; the job may still be running or may have been interrupted."}),
            )
        }
    }
}
