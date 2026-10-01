use super::*;

pub(in super::super) fn recover(
    history: &mut History<'_>,
    fixture: &Value,
) -> Result<(Value, Vec<Digest>), Error> {
    let target = history.target_slot;
    if fixture["target"]["targetSlot"] != target
        || history.genesis != "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
    {
        return Err(Error::new(
            "UNSUPPORTED_SLOT_HASHES_EVIDENCE",
            "vote-hash registry requires the matching mainnet boundary",
        ));
    }
    let executor = &fixture["runtime"]["binding"]["executor"];
    if !executor["m9Build"]["capabilities"]
        .as_array()
        .is_some_and(|v| v.iter().any(|c| c == "proven-slot-hashes-cache/v1"))
    {
        return Err(Error::new(
            "UNSUPPORTED_RUNTIME_CAPABILITY",
            "SlotHashes recovery requires proven-slot-hashes-cache/v1",
        ));
    }
    let id = executor["id"]
        .as_str()
        .ok_or_else(|| Error::new("WORKER_IDENTITY", "missing executor"))?;
    let bank = &fixture["runtime"]["bankContext"];
    let source_hash: Digest = serde_json::from_value(bank["blockSourceHash"].clone())
        .map_err(|e| Error::new("SOURCE_INTEGRITY", e.to_string()))?;
    let (_, target_block) = history.block(&source_hash)?;
    let hash = target_block["result"]["blockhash"]
        .as_str()
        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "missing target blockhash"))?;
    crate::shared::bank::context::assert_target_blockhash(bank, hash)?;
    let registry = Registry::bundled()?;
    if registry.profile(target)?.executor_source_id != id {
        return Err(Error::new(
            "UNSUPPORTED_SLOT_HASHES_EVIDENCE",
            "vote-hash target runtime differs from registry",
        ));
    }
    let mut missing = None;
    // A bounded later witness is evidence about the old Bank, not current state.
    // Grow only when the complete chain still lacks a successful vote witness.
    for distance in [32, 64, 128, 256, 512] {
        let start = target
            .checked_add(distance)
            .ok_or_else(|| Error::new("INVALID_REQUEST", "slot overflow"))?;
        match attempt(history, target, start, hash, id, &registry) {
            Ok(result) => return Ok(result),
            Err(error) if error.code == "UNSUPPORTED_SLOT_HASHES_EVIDENCE" => missing = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(Error::new(
        "UNSUPPORTED_SLOT_HASHES_EVIDENCE",
        "bounded finalized vote evidence cannot prove the complete SlotHashes",
    )
    .with_details(json!({"lookaheadsTried":[32,64,128,256,512],"lastError":missing})))
}

fn attempt(
    history: &mut History<'_>,
    target: u64,
    start: u64,
    hash: &str,
    id: &str,
    registry: &Registry,
) -> Result<(Value, Vec<Digest>), Error> {
    let mut proof = slot_hashes::Reconstruction::new(target, hash, id)?;
    let mut current = start;
    let mut ancestors = 0;
    let mut evidence = BTreeSet::new();
    for index in 0..=slot_hashes::ENTRIES * 2 {
        let mut record = history.discover_block(current);
        if index == 0
            && record.as_ref().err().is_some_and(|e| {
                e.code == "SOURCE_UNAVAILABLE"
                    || (e.code == "SOURCE_RPC_ERROR"
                        && e.details
                            .as_ref()
                            .is_some_and(|d| d["rpcCode"] == -32007 || d["rpcCode"] == -32009))
            })
        {
            // A missing later observer does not justify skipping an ancestor.
            // Try the known target itself; every traversed parent remains required.
            current = target;
            record = history.discover_block(current);
        }
        let record = record?;
        let hashes: Vec<Digest> = serde_json::from_value(record["evidenceHashes"].clone())
            .map_err(|e| Error::new("SOURCE_INTEGRITY", e.to_string()))?;
        evidence.extend(hashes);
        let bytes =
            data(record["value"]["rawBase64"].as_str().ok_or_else(|| {
                Error::new("SOURCE_INTEGRITY", "missing discovered block bytes")
            })?)?;
        let envelope = svm_replay_protocol::parse_json(&bytes)?;
        let block = &envelope["result"];
        let profile = registry.profile(current)?;
        proof.observe(current, block, &profile.executor_source_id)?;
        if current < target {
            ancestors += 1;
        }
        if ancestors == slot_hashes::ENTRIES {
            return Ok((proof.finish()?, evidence.into_iter().collect()));
        }
        current = block["parentSlot"]
            .as_u64()
            .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "missing parent slot"))?;
    }
    Err(Error::new(
        "UNSUPPORTED_SLOT_HASHES_EVIDENCE",
        "observer chain did not reach 512 proven target ancestors",
    ))
}
