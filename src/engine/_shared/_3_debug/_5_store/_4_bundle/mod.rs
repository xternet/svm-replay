use super::*;

pub(super) fn bundle(
    directory: &Path,
    mut manifest: SessionManifest,
    blobs: BTreeMap<String, Vec<u8>>,
) -> Result<SessionPin, Error> {
    require(
        directory.is_absolute(),
        "SESSION_CONFIG",
        "bundle destination must be absolute",
    )?;
    manifest.files = blobs
        .iter()
        .map(|(name, bytes)| {
            (
                name.clone(),
                FileReference {
                    sha256: Digest::of(bytes),
                    bytes: bytes.len() as u64,
                },
            )
        })
        .collect();
    manifest.session_identity = session_identity(&manifest)?;
    check_blobs(&manifest, &blobs)?;
    let bytes = encoded(&manifest)?;
    require(
        bytes.len() <= MAX_MANIFEST,
        "SESSION_LIMIT",
        "manifest exceeds bound",
    )?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(directory).map_err(|e| {
        error(
            "SESSION_IO",
            format!("create-only session {}: {e}", directory.display()),
        )
    })?;
    let result = (|| {
        for (name, bytes) in &blobs {
            write_file(
                &directory.join(name),
                bytes,
                matches!(
                    name.as_str(),
                    "reference-worker" | "capture-worker" | "worker-owner"
                ),
            )?;
        }
        // Manifest is the final commit point. Interrupted partial directories are
        // retained and never silently reused or overwritten.
        write_file(&directory.join("manifest.json"), &bytes, false)?;
        sync_directory(directory)?;
        Ok(SessionPin {
            manifest_sha256: Digest::of(&bytes),
            worker_sha256: manifest.capture.worker.sha256,
        })
    })();
    result.map_err(|error: Error| error.with_details(json!({"partialDirectoryRetained":directory})))
}
