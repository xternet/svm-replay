use super::*;

pub struct CacheContext<'a> {
    pub store: &'a mut Store,
    pub transport: &'a WorkerTransport,
    pub scratch_root: &'a Path,
    pub catalog_sha256: &'a Digest,
    pub implementation_sha256: &'a Digest,
}

#[derive(Clone, Copy)]
pub struct CacheOptions {
    pub result_cache: bool,
}

pub(in super::super) fn store_error(error: svm_replay_store::StoreError) -> Error {
    Error::new(format!("CACHE_STORE_{}", error.code), error.message)
        .with_details(json!({"retryable":error.retryable,"storeDetails":error.details}))
}

pub(in super::super) fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    value
        .get(name)
        .ok_or_else(|| Error::new("CACHE_PROTOCOL", format!("missing {name}")))
}

pub(in super::super) fn uint(value: &Value, name: &str) -> Result<u64, Error> {
    value
        .as_u64()
        .filter(|value| *value <= 9_007_199_254_740_991)
        .ok_or_else(|| {
            Error::new(
                "CACHE_PROTOCOL",
                format!("{name} is not an exact nonnegative integer"),
            )
        })
}

pub(in super::super) fn verify_worker(
    worker: &ResolvedWorker,
    budget: &ExecutionBudget,
) -> Result<(), Error> {
    budget.check().map_err(protocol_error)?;
    if file_sha256(&worker.spec.executable).map_err(protocol_error)? != worker.spec.sha256
        || worker.spec.sha256 != worker.descriptor.sha256.as_str()
    {
        return Err(Error::new(
            "WORKER_IDENTITY",
            "cache worker bytes/descriptor changed",
        ));
    }
    budget.check().map_err(protocol_error)
}

pub(in super::super) fn get(
    store: &mut Store,
    namespace: &'static str,
    key: &Digest,
    lease: &Value,
    held: &mut Vec<&'static str>,
) -> Result<Option<Vec<u8>>, Error> {
    let response = store
        .execute(json!({"version":1,"op":"get","namespace":namespace,"key":key,"lease":lease}))
        .map_err(store_error)?;
    if response["status"] == "MISS" {
        return Ok(None);
    }
    if response["status"] != "HIT" {
        return Err(Error::new(
            "CACHE_STORE_PROTOCOL",
            "get returned unknown status",
        ));
    }
    held.push(namespace);
    let encoded = field(&response, "dataBase64")?
        .as_str()
        .ok_or_else(|| Error::new("CACHE_STORE_PROTOCOL", "hit has no bytes"))?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| Error::new("CACHE_STORE_PROTOCOL", error.to_string()))?;
    if STANDARD.encode(&bytes) != encoded
        || response["sha256"] != json!(Digest::of(&bytes))
        || uint(field(&response, "sizeBytes")?, "blob size")? != bytes.len() as u64
    {
        return Err(Error::new(
            "CACHE_STORE_PROTOCOL",
            "hit byte identity differs",
        ));
    }
    Ok(Some(bytes))
}

pub(in super::super) fn put(
    store: &mut Store,
    namespace: &'static str,
    key: &Digest,
    bytes: &[u8],
    lease: &Value,
    held: &mut Vec<&'static str>,
) -> Result<(), Error> {
    let response = store.execute(json!({"version":1,"op":"put","namespace":namespace,"key":key,"dataBase64":STANDARD.encode(bytes),"lease":lease})).map_err(store_error)?;
    if response["status"] != "STORED" {
        return Err(Error::new(
            "CACHE_STORE_PROTOCOL",
            "put returned unknown status",
        ));
    }
    held.push(namespace);
    if response["sha256"] != json!(Digest::of(bytes))
        || uint(field(&response, "sizeBytes")?, "stored size")? != bytes.len() as u64
    {
        return Err(Error::new(
            "CACHE_STORE_PROTOCOL",
            "put acknowledgement differs",
        ));
    }
    Ok(())
}

pub(in super::super) fn write_checkpoint(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| Error::new("CHECKPOINT_IO", error.to_string()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| Error::new("CHECKPOINT_IO", error.to_string()))
}

pub(in super::super) fn verify(
    request: &PreparedRequest,
    fixture: &Value,
    block: &Value,
    output: &Value,
) -> Result<Value, Error> {
    let policy = match request.metadata_policy {
        MetadataPolicy::Strict => _3_verify_original::MetadataPolicy::Strict,
        MetadataPolicy::ArchivedComputeMeterWarning => {
            _3_verify_original::MetadataPolicy::ArchivedComputeMeterWarning
        }
    };
    _3_verify_original::verify_with_options(
        &request.candidate,
        fixture,
        block,
        output,
        policy,
        false,
    )
    .map(|verified| verified.0)
    .map_err(|error| Error::new("MISMATCH", error.to_string()))
}

pub(in super::super) fn response_receipt(response: &Value) -> Result<Value, Error> {
    let mut receipt = json!({});
    for name in [
        "schema",
        "status",
        "metrics",
        "reads",
        "checkpointSha256",
        "checkpointBytes",
        "pubkey",
        "guardedExecutionAttempted",
        "certifiedResult",
        "code",
        "reason",
        "detail",
        "targetSlot",
        "epoch",
        "read",
    ] {
        if let Some(value) = response.get(name) {
            receipt[name] = value.clone();
        }
    }
    if let Some(output) = response.get("output") {
        let bytes = serde_json::to_vec(output)
            .map_err(|error| Error::new("CACHE_PROTOCOL", error.to_string()))?;
        receipt["outputSha256"] = json!(Digest::of(&bytes));
        receipt["outputBytes"] = json!(bytes.len());
    }
    Ok(receipt)
}

pub fn execute(
    request: &PreparedRequest,
    worker: &ResolvedWorker,
    context: CacheContext<'_>,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    options: CacheOptions,
) -> Result<Value, Error> {
    execute_inner(request, worker, context, limits, budget, options, None)
}
