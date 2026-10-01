use super::*;

pub(in super::super) fn recover(
    context: &Recovery<'_>,
    history: &mut History<'_>,
    key: &str,
) -> Result<Option<Value>, Error> {
    let registry = Registry::bundled()?.lifecycle_catalog();
    for kind in ["builtins", "precompiles"] {
        if array(&registry[kind], "native programs")?
            .iter()
            .any(|p| p["programId"] == key)
        {
            return Ok(None); // Existing Bank initialization remains responsible.
        }
    }
    if history.genesis != "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d" {
        return Err(unsupported("loader lifecycle registry is mainnet-specific"));
    }
    let executor = context.binding["executor"]["id"]
        .as_str()
        .ok_or_else(|| invalid("executor identity"))?;
    let mut hashes = BTreeSet::new();
    let mut anchor = None;
    let slots = anchor_slots(&registry, context.a.slot, executor)?;
    for &slot in &slots {
        let value = observe(history, key, slot, &mut hashes)?;
        if value["presence"] == "present" {
            anchor = Some(value);
            break;
        }
    }
    let anchor = anchor.ok_or_else(|| {
        unsupported("bounded historical anchors have no Program image")
            .with_details(json!({"pubkey":key,"anchorSlotsTried":slots}))
    })?;
    let data_key = program_data_address(&anchor)?
        .ok_or_else(|| unsupported("anchor is not an upgradeable Program"))?;
    let code = observe(history, &data_key, context.a.parent, &mut hashes)?;
    let bytes = account_data(&code)?;
    let deployed = bytes
        .get(4..12)
        .ok_or_else(|| unsupported("historical ProgramData header missing"))?;
    let witness_slot = u64::from_le_bytes(
        deployed
            .try_into()
            .map_err(|_| unsupported("deployment slot"))?,
    );
    if witness_slot >= context.a.parent {
        return Err(unsupported("deployment witness must precede the parent"));
    }
    let observed = history.discover_block(witness_slot)?;
    let witness_hashes: Vec<Digest> = serde_json::from_value(observed["evidenceHashes"].clone())
        .map_err(|e| Error::new("SOURCE_INTEGRITY", e.to_string()))?;
    if witness_hashes.is_empty() {
        return Err(Error::new("SOURCE_INTEGRITY", "witness has no provenance"));
    }
    hashes.extend(witness_hashes);
    let raw = data(
        observed["value"]["rawBase64"]
            .as_str()
            .ok_or_else(|| invalid("witness block bytes"))?,
    )?;
    let witness = svm_replay_protocol::parse_json(&raw)?;
    let rent = observe(history, RENT, context.a.parent, &mut hashes)?;
    let input = json!({"program":key,"parentSlot":context.a.parent,"targetSlot":context.a.slot,
        "anchor":anchor,"programdata":code,"rent":rent,"witnessSlot":witness_slot,
        "witnessBlock":witness["result"],"targetBlock":context.block,"registry":registry});
    let mut result = program_header::derive(&input).map_err(|error| unsupported(error.message))?;
    result["proof"]["evidenceHashes"] = json!(hashes);
    Ok(Some(result))
}
