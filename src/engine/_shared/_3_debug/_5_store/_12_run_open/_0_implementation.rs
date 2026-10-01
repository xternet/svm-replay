use super::*;

pub fn open_run(
    directory: &Path,
    manifest_sha256: &Digest,
    session: &SessionPin,
) -> Result<SavedRun, Error> {
    let metadata =
        fs::symlink_metadata(directory).map_err(|e| error("SESSION_IO", e.to_string()))?;
    require(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "SESSION_INTEGRITY",
        "run root is not a real directory",
    )?;
    let directory = directory
        .canonicalize()
        .map_err(|e| error("SESSION_IO", e.to_string()))?;
    let bytes =
        read_bounded_file(&directory.join("run.json"), MAX_MANIFEST).map_err(protocol_error)?;
    require(
        Digest::of(&bytes) == *manifest_sha256,
        "SESSION_INTEGRITY",
        "run manifest differs from external pin",
    )?;
    let manifest: RunManifest = serde_json::from_value(parse_json(&bytes)?)
        .map_err(|e| error("SESSION_FORMAT", e.to_string()))?;
    require(
        &manifest.session == session,
        "SESSION_IDENTITY",
        "run belongs to a different pinned session",
    )?;
    manifest.bounds.validate(&manifest.policy)?;
    let names = run_names(&manifest)?;
    require(
        manifest.files.keys().cloned().collect::<BTreeSet<_>>() == names,
        "SESSION_FORMAT",
        "run manifest file set differs",
    )?;
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(&directory).map_err(|e| error("SESSION_IO", e.to_string()))? {
        actual.insert(
            entry
                .map_err(|e| error("SESSION_IO", e.to_string()))?
                .file_name()
                .into_string()
                .map_err(|_| error("SESSION_FORMAT", "non-UTF8 run file name"))?,
        );
    }
    let mut expected = names.clone();
    expected.insert("run.json".into());
    require(
        actual == expected,
        "SESSION_FORMAT",
        "unexpected or missing run directory artifacts",
    )?;
    let mut blobs = BTreeMap::new();
    let mut total = 0usize;
    for name in names {
        let reference = &manifest.files[&name];
        let maximum = run_limit(&name, &manifest.policy)?;
        require(
            reference.bytes <= maximum as u64,
            "SESSION_LIMIT",
            "run artifact size exceeds limit",
        )?;
        total = total
            .checked_add(reference.bytes as usize)
            .ok_or_else(|| error("SESSION_LIMIT", "run size overflow"))?;
        require(
            total <= MAX_BUNDLE,
            "SESSION_LIMIT",
            "run bundle size limit",
        )?;
        blobs.insert(
            name.clone(),
            read_bounded_file(&directory.join(&name), (reference.bytes as usize).max(1))
                .map_err(protocol_error)?,
        );
    }
    run_blobs(&manifest, &blobs)?;
    let mut exports = Vec::new();
    for phase in manifest.exports.keys() {
        exports.push((
            phase.clone(),
            crate::shared::trace::CaptureExport {
                artifact: parse_json(&blobs[&format!("{phase}-artifact.json")])?,
                payload: blobs[&format!("{phase}-capture.json")].clone(),
            },
        ));
    }
    Ok(SavedRun {
        directory,
        manifest_sha256: manifest_sha256.clone(),
        output: parse_json(&blobs["output.json"])?,
        verification: parse_json(&blobs["verification.json"])?,
        control_evidence: parse_json(&blobs["control-evidence.json"])?,
        events: serde_json::from_value(parse_json(&blobs["events.json"])?)
            .map_err(|e| error("SESSION_FORMAT", e.to_string()))?,
        receipt: parse_json(&blobs["receipt.json"])?,
        exports,
        manifest,
    })
}
