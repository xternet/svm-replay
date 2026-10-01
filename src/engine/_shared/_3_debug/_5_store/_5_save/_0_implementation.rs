use super::*;

pub fn save_session(directory: &Path, options: &SaveOptions<'_>) -> Result<SessionPin, Error> {
    require(
        options.reference.sha256 == options.reference_descriptor.sha256.as_str()
            && options.capture.sha256 == options.capture_descriptor.worker.sha256.as_str(),
        "SESSION_IDENTITY",
        "worker spec/descriptor pins differ",
    )?;
    validate_request(options.request, options.reference_descriptor)?;
    let mut blobs = BTreeMap::new();
    blobs.insert("request.json".into(), encoded(options.request)?);
    blobs.insert(
        "control-request.json".into(),
        encoded(&control(options.request)?)?,
    );
    for (name, path, expected) in [
        (
            "reference-worker",
            &options.reference.executable,
            &options.reference.sha256,
        ),
        (
            "capture-worker",
            &options.capture.executable,
            &options.capture.sha256,
        ),
        (
            "worker-owner",
            &options.owner.executable,
            &options.owner.sha256,
        ),
    ] {
        let bytes = read_bounded_file(path, MAX_FILE).map_err(protocol_error)?;
        require(
            Digest::of(&bytes).as_str() == expected
                && file_sha256(path).map_err(protocol_error)? == *expected,
            "SESSION_INTEGRITY",
            "binary differs from supplied immutable pin",
        )?;
        blobs.insert(name.into(), bytes);
    }
    let symbols = if let Some((path, pin)) = options.symbols {
        let bytes = read_bounded_file(path, MAX_SYMBOLS).map_err(protocol_error)?;
        require(
            Digest::of(&bytes) == *pin && bytes.starts_with(b"\x7fELF"),
            "SESSION_INTEGRITY",
            "symbol ELF/hash mismatch",
        )?;
        blobs.insert("symbols.elf".into(), bytes);
        SessionSymbols {
            status: "exact-elf; source-availability-unchecked".into(),
            elf_sha256: Some(pin.clone()),
        }
    } else {
        SessionSymbols {
            status: "assembly-only".into(),
            elf_sha256: None,
        }
    };
    let mut reference = options.reference_descriptor.clone();
    reference.file = "reference-worker".into();
    let mut capture = options.capture_descriptor.clone();
    capture.worker.file = "capture-worker".into();
    let manifest = SessionManifest {
        schema: "svm-m17-saved-debug-session/v1".into(),
        assurance: ASSURANCE.into(),
        reference,
        capture: capture.clone(),
        owner_sha256: Digest::new(&options.owner.sha256)?,
        implementation_sha256: options.implementation_sha256.clone(),
        policy: options.policy.clone(),
        bounds: options.bounds.clone(),
        capture_identity: capture_identity(
            &Digest::of(canonical_json(&options.request.fixture)),
            &capture.worker,
            options.policy,
            options.bounds,
        )?,
        session_identity: Digest::of(b"pending complete manifest identity"),
        symbols,
        lineage: None,
        files: BTreeMap::new(),
    };
    bundle(directory, manifest, blobs)
}
