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

fn binding(
    slot: u64,
    catalog: &InstalledCatalog,
    registry: &Registry,
) -> Result<(String, Value), Error> {
    let profile = registry.profile(slot)?;
    let worker = catalog
        .manifest
        .workers
        .iter()
        .find(|worker| worker.executor_source_id == profile.executor_source_id)
        .ok_or_else(|| {
            Error::new(
                "UNSUPPORTED_RUNTIME",
                "install the runtime required for this slot",
            )
        })?;
    Ok((worker.family.clone(), registry.bind(slot, worker)?))
}

fn raw_block(record: &Value) -> Result<Vec<u8>, Error> {
    let bytes = record
        .pointer("/provenance/responseBodyBase64")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "block evidence missing"))?;
    STANDARD
        .decode(bytes)
        .map_err(|e| Error::new("SOURCE_INTEGRITY", e.to_string()))
}

pub fn prepare_initialization_with_registry(
    after: u64,
    source: &AlchemySource,
    catalog: &InstalledCatalog,
    limits: Limits,
    registry: &Registry,
) -> Result<crate::shared::historical::InitializationRequest, Error> {
    limits.validate()?;
    let record = source.discover_next_block(after).map_err(source_error)?;
    let slot = record["query"]["slot"]
        .as_u64()
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "selected produced slot missing"))?;
    let (family, runtime_binding) = binding(slot, catalog, registry)?;
    let raw = raw_block(&record)?;
    let request = crate::shared::historical::InitializationRequest {
        schema: "svm-historical-initialization-request/v1".into(),
        request_id: format!("after-slot-{after}"),
        family,
        genesis_hash: record["query"]["genesisHash"]
            .as_str()
            .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "initialization source network missing"))?
            .into(),
        after_slot: after,
        slot,
        raw_block_base64: STANDARD.encode(&raw),
        block_sha256: svm_replay_protocol::Digest::of(&raw),
        runtime_binding,
        extra_accounts: Vec::new(),
        limits,
    };
    request.validate()?;
    Ok(request)
}

fn from_block(
    signature: &str,
    record: &Value,
    family: String,
    binding: Value,
    replacement: Option<String>,
    overrides: Option<Value>,
    limits: Limits,
) -> Result<HistoricalRequest, Error> {
    let raw = raw_block(record)?;
    let genesis_hash = record["query"]["genesisHash"]
        .as_str()
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "source network missing"))?
        .to_owned();
    let request = HistoricalRequest {
        schema: "svm-replay-historical/v1".into(),
        request_id: format!("tx-{signature}"),
        family,
        genesis_hash,
        candidate: boundary(&raw, signature, &binding)?,
        runtime_binding: binding,
        replacement_transaction_base64: replacement,
        requested_account_overrides: overrides,
        what_if: None,
        extra_accounts: Vec::new(),
        after_slot: None,
        bank_inputs: Vec::new(),
        metadata_policy: MetadataPolicy::ArchivedComputeMeterWarning,
        limits,
    };
    request.validate()?;
    Ok(request)
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
    crate::shared::progress::phase(crate::shared::progress::Phase::LocatingTransaction);
    let lookup = source
        .discover_transaction(signature)
        .map_err(source_error)?;
    let slot = lookup["slot"]
        .as_u64()
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "lookup slot missing"))?;
    // Keep runtime admission ahead of an otherwise unnecessary block acquisition.
    let (family, binding) = binding(slot, catalog, registry)?;
    let record = source.discover_block(slot).map_err(source_error)?;
    from_block(
        signature,
        &record,
        family,
        binding,
        replacement,
        overrides,
        limits,
    )
}

pub fn prepare_after_slot_with_registry(
    after: u64,
    source: &AlchemySource,
    catalog: &InstalledCatalog,
    replacement: Option<String>,
    overrides: Option<Value>,
    limits: Limits,
    registry: &Registry,
) -> Result<HistoricalRequest, Error> {
    limits.validate()?;
    crate::shared::progress::phase(crate::shared::progress::Phase::LocatingTransaction);
    let record = source.discover_next_block(after).map_err(source_error)?;
    let slot = record["query"]["slot"]
        .as_u64()
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "selected produced slot missing"))?;
    let (family, binding) = binding(slot, catalog, registry)?;
    let raw = raw_block(&record)?;
    let block = svm_replay_protocol::parse_json(&raw)?;
    let transactions = block["result"]["transactions"]
        .as_array()
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "full block transactions missing"))?;
    let first = transactions.first().ok_or_else(|| Error::new("UNSUPPORTED_HISTORICAL_BOUNDARY",
        format!("produced slot {slot} is empty; initialized empty-block boundaries are not available yet")))?;
    let signature = first
        .pointer("/transaction/signatures/0")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "first archived signature missing"))?;
    let mut request = from_block(
        signature,
        &record,
        family,
        binding,
        replacement,
        overrides,
        limits,
    )?;
    request.after_slot = Some(after);
    request.request_id = format!("after-slot-{after}");
    request.validate()?;
    Ok(request)
}
