use super::*;

pub(in super::super) fn load(
    directory: &Path,
    pin: &SessionPin,
) -> Result<(PathBuf, SessionManifest, BTreeMap<String, Vec<u8>>), Error> {
    let metadata =
        fs::symlink_metadata(directory).map_err(|e| error("SESSION_IO", e.to_string()))?;
    require(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "SESSION_INTEGRITY",
        "session root is not a real directory",
    )?;
    let directory = directory
        .canonicalize()
        .map_err(|e| error("SESSION_IO", e.to_string()))?;
    let bytes = read_bounded_file(&directory.join("manifest.json"), MAX_MANIFEST)
        .map_err(protocol_error)?;
    require(
        Digest::of(&bytes) == pin.manifest_sha256,
        "SESSION_INTEGRITY",
        "manifest differs from independently supplied pin",
    )?;
    let manifest: SessionManifest = serde_json::from_value(parse_json(&bytes)?)
        .map_err(|e| error("SESSION_FORMAT", e.to_string()))?;
    policy(&manifest)?;
    require(
        manifest.capture.worker.sha256 == pin.worker_sha256,
        "SESSION_IDENTITY",
        "external capture worker pin differs",
    )?;
    let expected = expected_names(&manifest);
    require(
        manifest.files.keys().cloned().collect::<BTreeSet<_>>() == expected,
        "SESSION_FORMAT",
        "unexpected manifest file names",
    )?;
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(&directory).map_err(|e| error("SESSION_IO", e.to_string()))? {
        let entry = entry.map_err(|e| error("SESSION_IO", e.to_string()))?;
        actual.insert(
            entry
                .file_name()
                .into_string()
                .map_err(|_| error("SESSION_FORMAT", "non-UTF8 artifact name"))?,
        );
    }
    let mut expected_directory = expected.clone();
    expected_directory.insert("manifest.json".into());
    require(
        actual == expected_directory,
        "SESSION_FORMAT",
        "unexpected or missing directory artifacts",
    )?;
    let mut blobs = BTreeMap::new();
    let mut total = 0usize;
    for name in expected {
        let reference = &manifest.files[&name];
        let maximum = if name == "symbols.elf" {
            MAX_SYMBOLS
        } else {
            MAX_FILE
        };
        require(
            reference.bytes <= maximum as u64,
            "SESSION_LIMIT",
            "manifest artifact size exceeds bound",
        )?;
        total = total
            .checked_add(reference.bytes as usize)
            .ok_or_else(|| error("SESSION_LIMIT", "bundle size overflow"))?;
        require(
            total <= MAX_BUNDLE,
            "SESSION_LIMIT",
            "bundle exceeds total bound",
        )?;
        let bytes = read_bounded_file(&directory.join(&name), (reference.bytes as usize).max(1))
            .map_err(protocol_error)?;
        blobs.insert(name, bytes);
    }
    check_blobs(&manifest, &blobs)?;
    Ok((directory, manifest, blobs))
}

pub fn open_session(directory: &Path, pin: &SessionPin) -> Result<SavedSession, Error> {
    let (directory, manifest, blobs) = load(directory, pin)?;
    let request = PreparedRequest::parse(&blobs["request.json"])?;
    let control = PreparedRequest::parse(&blobs["control-request.json"])?;
    Ok(SavedSession {
        reference: WorkerSpec {
            executable: directory.join("reference-worker"),
            sha256: manifest.reference.sha256.as_str().into(),
        },
        capture: WorkerSpec {
            executable: directory.join("capture-worker"),
            sha256: manifest.capture.worker.sha256.as_str().into(),
        },
        owner: ProcessOwner {
            executable: directory.join("worker-owner"),
            sha256: manifest.owner_sha256.as_str().into(),
        },
        directory,
        pin: pin.clone(),
        manifest,
        request,
        control,
    })
}
