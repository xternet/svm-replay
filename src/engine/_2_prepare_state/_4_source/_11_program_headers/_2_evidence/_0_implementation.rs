use super::*;

pub(in super::super) fn preexisting(block: &Value, key: &str) -> Result<bool, Error> {
    for tx in array(&block["transactions"], "historical transactions")? {
        let mut keys = strings(&tx["transaction"]["message"]["accountKeys"])?;
        if let Some(loaded) = tx["meta"].get("loadedAddresses").filter(|v| !v.is_null()) {
            keys.extend(strings(&loaded["writable"])?);
            keys.extend(strings(&loaded["readonly"])?);
        }
        let balances = array(&tx["meta"]["preBalances"], "historical balances")?;
        if balances.len() != keys.len() {
            return Err(invalid("balance/key count differs"));
        }
        if let Some(index) = keys.iter().position(|k| k == key) {
            return crate::shared::diff::exact_u64(&balances[index])
                .map(|n| n != 0)
                .map_err(invalid);
        }
    }
    Ok(false) // No first-touch evidence: ordinary absence handling stays in force.
}

pub(in super::super) fn observe(
    history: &mut History<'_>,
    key: &str,
    slot: u64,
    hashes: &mut BTreeSet<Digest>,
) -> Result<Value, Error> {
    // Explicit evidence query. History::account still rejects future images as
    // execution state; this observation retains its real future sourceSlot.
    let observed = history.read(&json!({"kind":"account","genesisHash":history.genesis,
        "slot":slot,"pubkey":key,"phase":"end-slot"}))?;
    let evidence: Vec<Digest> = serde_json::from_value(observed["evidenceHashes"].clone())
        .map_err(|e| Error::new("SOURCE_INTEGRITY", e.to_string()))?;
    if evidence.is_empty() {
        return Err(Error::new(
            "SOURCE_INTEGRITY",
            "header input has no provenance",
        ));
    }
    hashes.extend(evidence);
    classify_account(observed["value"].clone(), key, slot, &Roles::default())
}

pub(in super::super) fn anchor_slots(
    registry: &Value,
    target: u64,
    executor: &str,
) -> Result<Vec<u64>, Error> {
    let reviewed = ["litesvm-v0.6.1-agave-2.2.20", "litesvm-v0.7.1-agave-2.3.9"];
    let mut end: Option<u64> = None;
    for profile in array(&registry["profiles"], "lifecycle profiles")? {
        let first = profile["earliestSlot"]
            .as_u64()
            .ok_or_else(|| invalid("profile start"))?;
        let last = profile["latestSlot"]
            .as_u64()
            .ok_or_else(|| invalid("profile end"))?;
        if last < target {
            continue;
        }
        let id = profile["executorSourceId"]
            .as_str()
            .ok_or_else(|| invalid("profile executor"))?;
        if end.is_none() && (first > target || id != executor) {
            return Err(unsupported(
                "target does not match reviewed lifecycle registry",
            ));
        }
        if !reviewed.contains(&id) {
            break;
        }
        if let Some(previous) = end {
            if previous.checked_add(1) != Some(first) {
                break;
            }
        }
        end = Some(last);
    }
    let end = end.ok_or_else(|| unsupported("unreviewed loader lifecycle"))?;
    let mut slots = BTreeSet::new();
    // Bounded historical probes, never a current-account endpoint. The interval
    // ends before any unreviewed native loader version.
    for offset in [
        432_000u64, 864_000, 1_728_000, 3_456_000, 6_912_000, 13_824_000,
    ] {
        let slot = target
            .checked_add(offset)
            .ok_or_else(|| invalid("anchor slot overflow"))?
            .min(end);
        if slot >= target {
            slots.insert(slot);
        }
    }
    Ok(slots.into_iter().collect())
}
