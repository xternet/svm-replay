use super::*;

pub fn read(path: &Path) -> Result<Value, Error> {
    let bytes = read_bounded_file(path, 256 * 1024 * 1024)
        .map_err(|e| Error::new("REQUEST_IO", e.to_string()))?;
    parse_json(&bytes)
}

pub fn validated(value: Value) -> Result<Value, Error> {
    let bytes =
        serde_json::to_vec(&value).map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
    let canonical = match value["schema"].as_str() {
        Some("svm-replay-historical/v1") => serde_json::to_value(HistoricalRequest::parse(&bytes)?),
        Some("svm-replay-prepared/v1") => serde_json::to_value(PreparedRequest::parse(&bytes)?),
        _ => return Err(Error::new("INVALID_REQUEST", "unknown request schema")),
    }
    .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
    Ok(canonical)
}

pub fn inspect(path: &Path) -> Result<Value, Error> {
    let request = validated(read(path)?)?;
    let bytes =
        serde_json::to_vec(&request).map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
    Ok(
        json!({"outcome":"COMPLETED","schema":"svm-replay-validation/v1","requestSchema":request["schema"],"requestId":request["requestId"],"requestIdentity":Digest::of(bytes),"limits":request["limits"],"executionPerformed":false}),
    )
}

pub fn save(path: &Path, value: &Value) -> Result<Digest, Error> {
    let bytes =
        serde_json::to_vec(value).map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
    save_bytes(path, &bytes)
}

pub fn save_bytes(path: &Path, bytes: &[u8]) -> Result<Digest, Error> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| Error::new("REQUEST_IO", e.to_string()))?;
    file.write_all(bytes)
        .map_err(|e| Error::new("REQUEST_IO", e.to_string()))?;
    file.sync_all()
        .map_err(|e| Error::new("REQUEST_IO", e.to_string()))?;
    Ok(Digest::of(bytes))
}

pub fn build(
    candidate: Value,
    archived_block: Option<Value>,
    runtime_binding: Value,
    family: String,
    genesis_hash: String,
    request_id: String,
    replacement: Option<String>,
    overrides: Option<Value>,
    bank_inputs: Vec<String>,
    limits: Limits,
    output: &Path,
) -> Result<Value, Error> {
    let request = HistoricalRequest {
        schema: "svm-replay-historical/v1".into(),
        request_id,
        family,
        genesis_hash,
        candidate,
        runtime_binding,
        replacement_transaction_base64: replacement,
        requested_account_overrides: overrides,
        bank_inputs,
        metadata_policy: MetadataPolicy::Strict,
        limits,
    };
    request.validate()?;
    if let Some(block) = archived_block {
        svm_replay_engine::_2_prepare_state::analysis::analyze(&request, &block)?;
    }
    let value =
        serde_json::to_value(&request).map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
    let value = validated(value)?;
    let digest = save(output, &value)?;
    Ok(
        json!({"outcome":"COMPLETED","schema":"svm-replay-request-built/v1","requestPath":output,"sha256":digest,"metadataPolicy":"STRICT"}),
    )
}
