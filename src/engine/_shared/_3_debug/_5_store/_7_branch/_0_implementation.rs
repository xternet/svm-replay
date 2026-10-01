use super::*;

pub(in super::super) fn variant(request: &mut PreparedRequest, value: &Value) -> Result<(), Error> {
    let object = value
        .as_object()
        .ok_or_else(|| error("SESSION_VARIANT", "variant must be an object"))?;
    require(
        !object.is_empty()
            && object.keys().all(|k| {
                matches!(
                    k.as_str(),
                    "replacementTransactionBase64" | "requestedAccountOverrides"
                )
            }),
        "SESSION_VARIANT",
        "only explicit replacement and account override fields may change",
    )?;
    if let Some(replacement) = object.get("replacementTransactionBase64") {
        let encoded = replacement
            .as_str()
            .ok_or_else(|| error("SESSION_VARIANT", "replacement is not base64 string"))?;
        transaction::decode(encoded)?;
        request.fixture["target"]["replacementTransactionBase64"] = replacement.clone();
    }
    if let Some(overrides) = object.get("requestedAccountOverrides") {
        let rows = overrides
            .as_array()
            .ok_or_else(|| error("SESSION_VARIANT", "override list is not an array"))?;
        require(
            !rows.is_empty() && rows.len() <= 256,
            "SESSION_VARIANT",
            "override list limit",
        )?;
        let mut keys = BTreeSet::new();
        for row in rows {
            let row = row
                .as_object()
                .ok_or_else(|| error("SESSION_VARIANT", "override is not object"))?;
            require(
                row.keys()
                    .all(|key| matches!(key.as_str(), "pubkey" | "lamports" | "dataBase64"))
                    && (row.contains_key("lamports") || row.contains_key("dataBase64")),
                "SESSION_VARIANT",
                "invalid override fields",
            )?;
            let key = row
                .get("pubkey")
                .and_then(Value::as_str)
                .ok_or_else(|| error("SESSION_VARIANT", "override pubkey missing"))?;
            let decoded = bs58::decode(key)
                .into_vec()
                .map_err(|e| error("SESSION_VARIANT", e.to_string()))?;
            require(
                decoded.len() == 32
                    && bs58::encode(decoded).into_string() == key
                    && keys.insert(key),
                "SESSION_VARIANT",
                "invalid or duplicate override key",
            )?;
            if let Some(lamports) = row.get("lamports") {
                let text = lamports
                    .as_str()
                    .ok_or_else(|| error("SESSION_VARIANT", "lamports must be decimal string"))?;
                let parsed = text
                    .parse::<u64>()
                    .map_err(|e| error("SESSION_VARIANT", e.to_string()))?;
                require(
                    parsed.to_string() == text,
                    "SESSION_VARIANT",
                    "lamports not canonical u64",
                )?;
            }
            if let Some(data) = row.get("dataBase64") {
                let text = data.as_str().ok_or_else(|| {
                    error("SESSION_VARIANT", "override data must be base64 string")
                })?;
                let bytes = STANDARD
                    .decode(text)
                    .map_err(|e| error("SESSION_VARIANT", e.to_string()))?;
                require(
                    STANDARD.encode(bytes) == text,
                    "SESSION_VARIANT",
                    "account data not canonical base64",
                )?;
            }
        }
        request.fixture["requestedAccountOverrides"] = overrides.clone();
    }
    Ok(())
}

pub fn branch_session(
    parent_directory: &Path,
    pin: &SessionPin,
    directory: &Path,
    changes: &Value,
) -> Result<SessionPin, Error> {
    let (parent_directory, mut manifest, mut blobs) = load(parent_directory, pin)?;
    let mut request = PreparedRequest::parse(&blobs["request.json"])?;
    variant(&mut request, changes)?;
    let parent = read_bounded_file(&parent_directory.join("manifest.json"), MAX_MANIFEST)
        .map_err(protocol_error)?;
    require(
        Digest::of(&parent) == pin.manifest_sha256,
        "SESSION_INTEGRITY",
        "parent manifest changed during branching",
    )?;
    manifest.lineage = Some(SessionLineage {
        kind: "hypothetical".into(),
        parent_manifest_sha256: pin.manifest_sha256.clone(),
        parent_session_identity: manifest.session_identity.clone(),
    });
    manifest.capture_identity = capture_identity(
        &Digest::of(canonical_json(&request.fixture)),
        &manifest.capture.worker,
        &manifest.policy,
        &manifest.bounds,
    )?;
    blobs.insert("parent-manifest.json".into(), parent);
    blobs.insert("request.json".into(), encoded(&request)?);
    bundle(directory, manifest, blobs)
}
