use super::*;

pub(in super::super) fn expected_names(manifest: &SessionManifest) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = [
        "request.json",
        "control-request.json",
        "reference-worker",
        "capture-worker",
        "worker-owner",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if manifest.symbols.elf_sha256.is_some() {
        names.insert("symbols.elf".into());
    }
    if manifest.lineage.is_some() {
        names.insert("parent-manifest.json".into());
    }
    names
}

pub(in super::super) fn check_blobs(
    manifest: &SessionManifest,
    blobs: &BTreeMap<String, Vec<u8>>,
) -> Result<(), Error> {
    policy(manifest)?;
    let names = expected_names(manifest);
    require(
        manifest.files.keys().cloned().collect::<BTreeSet<_>>() == names
            && blobs.keys().cloned().collect::<BTreeSet<_>>() == names,
        "SESSION_FORMAT",
        "unexpected or missing bundle artifacts",
    )?;
    let mut total = 0usize;
    for (name, bytes) in blobs {
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| error("SESSION_LIMIT", "bundle size overflow"))?;
        require(
            bytes.len()
                <= if name == "symbols.elf" {
                    MAX_SYMBOLS
                } else {
                    MAX_FILE
                }
                && total <= MAX_BUNDLE,
            "SESSION_LIMIT",
            "bundle/file size bound exceeded",
        )?;
        let expected = &manifest.files[name];
        require(
            expected.bytes == bytes.len() as u64 && expected.sha256 == Digest::of(bytes),
            "SESSION_INTEGRITY",
            &format!("artifact hash/size differs: {name}"),
        )?;
    }
    require(
        manifest.files["reference-worker"].sha256 == manifest.reference.sha256
            && manifest.files["capture-worker"].sha256 == manifest.capture.worker.sha256
            && manifest.files["worker-owner"].sha256 == manifest.owner_sha256,
        "SESSION_IDENTITY",
        "bundled binary differs from pinned descriptor",
    )?;
    let request = PreparedRequest::parse(&blobs["request.json"])?;
    let original = PreparedRequest::parse(&blobs["control-request.json"])?;
    validate_request(&request, &manifest.reference)?;
    validate_request(&original, &manifest.reference)?;
    require(
        encoded(&control(&request)?)? == encoded(&original)?,
        "SESSION_LINEAGE",
        "branch changes fields outside explicit requested variants or loses root control",
    )?;
    require(
        manifest.capture_identity
            == capture_identity(
                &Digest::of(canonical_json(&request.fixture)),
                &manifest.capture.worker,
                &manifest.policy,
                &manifest.bounds,
            )?
            && manifest.session_identity == session_identity(manifest)?,
        "SESSION_IDENTITY",
        "derived session/capture identity differs",
    )?;
    match &manifest.symbols.elf_sha256 {
        None => require(
            manifest.symbols.status == "assembly-only",
            "SESSION_FORMAT",
            "invalid assembly-only symbols declaration",
        )?,
        Some(expected) => {
            let bytes = &blobs["symbols.elf"];
            require(
                manifest.symbols.status == "exact-elf; source-availability-unchecked"
                    && Digest::of(bytes) == *expected
                    && bytes.starts_with(b"\x7fELF"),
                "SESSION_INTEGRITY",
                "saved exact symbol ELF differs",
            )?;
        }
    }
    if let Some(lineage) = &manifest.lineage {
        let bytes = &blobs["parent-manifest.json"];
        require(
            Digest::of(bytes) == lineage.parent_manifest_sha256,
            "SESSION_LINEAGE",
            "parent manifest pin differs",
        )?;
        let parent: SessionManifest = serde_json::from_value(parse_json(bytes)?)
            .map_err(|e| error("SESSION_FORMAT", e.to_string()))?;
        policy(&parent)?;
        require(
            parent.session_identity == session_identity(&parent)?
                && parent.session_identity == lineage.parent_session_identity
                && parent
                    .files
                    .get("control-request.json")
                    .is_some_and(|file| {
                        file.sha256 == manifest.files["control-request.json"].sha256
                    }),
            "SESSION_LINEAGE",
            "branch root-control lineage differs",
        )?;
    }
    Ok(())
}
