use super::*;

pub fn prepare(
    signature: &str,
    source: &AlchemySource,
    catalog: &InstalledCatalog,
    replacement: Option<String>,
    overrides: Option<Value>,
    limits: Limits,
) -> Result<HistoricalRequest, Error> {
    prepare_with_registry(
        signature,
        source,
        catalog,
        replacement,
        overrides,
        limits,
        &Registry::bundled()?,
    )
}

pub fn prepare_with_registry(
    signature: &str,
    source: &AlchemySource,
    catalog: &InstalledCatalog,
    replacement: Option<String>,
    overrides: Option<Value>,
    limits: Limits,
    registry: &Registry,
) -> Result<HistoricalRequest, Error> {
    limits.validate()?;
    let lookup = source
        .discover_transaction(signature)
        .map_err(source_error)?;
    let slot = lookup["slot"]
        .as_u64()
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "lookup slot missing"))?;
    let profile = registry.profile(slot)?;
    let worker = catalog
        .manifest
        .workers
        .iter()
        .find(|w| w.executor_source_id == profile.executor_source_id)
        .ok_or_else(|| {
            Error::new(
                "UNSUPPORTED_RUNTIME",
                "install the runtime required for this slot",
            )
        })?;
    let binding = registry.bind(slot, worker)?;
    let block = source.discover_block(slot).map_err(source_error)?;
    let raw = block
        .pointer("/provenance/responseBodyBase64")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "block evidence missing"))?;
    let raw = STANDARD
        .decode(raw)
        .map_err(|e| Error::new("SOURCE_INTEGRITY", e.to_string()))?;
    let genesis_hash = source.identity()["genesisHash"]
        .as_str()
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "source network missing"))?
        .to_owned();
    let request = HistoricalRequest {
        schema: "svm-replay-historical/v1".into(),
        request_id: format!("tx-{signature}"),
        family: worker.family.clone(),
        genesis_hash,
        candidate: boundary(&raw, signature, &binding)?,
        runtime_binding: binding,
        replacement_transaction_base64: replacement,
        requested_account_overrides: overrides,
        bank_inputs: Vec::new(),
        metadata_policy: MetadataPolicy::ArchivedComputeMeterWarning,
        limits,
    };
    request.validate()?;
    Ok(request)
}
