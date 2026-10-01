use serde_json::{json, Value};
use svm_replay_protocol::{parse_json, Digest, Error};

/// Derive an index and byte pins; runtime and block semantics are validated before writing.
pub fn boundary(raw: &[u8], signature: &str, binding: &Value) -> Result<Value, Error> {
    crate::shared::history::address(signature, 64)?;
    let envelope = parse_json(raw)?;
    let invalid = || {
        Error::new(
            "HISTORICAL_BOUNDARY",
            "full block with one matching signature required",
        )
    };
    let slot = binding["targetSlot"].as_u64().ok_or_else(invalid)?;
    if envelope.get("error").is_some()
        || envelope["result"]["parentSlot"]
            .as_u64()
            .is_none_or(|parent| parent >= slot)
    {
        return Err(invalid());
    }
    let transactions = envelope["result"]["transactions"]
        .as_array()
        .ok_or_else(invalid)?;
    let mut matching = Vec::new();
    for (index, tx) in transactions.iter().enumerate() {
        let first = tx["transaction"]["signatures"][0]
            .as_str()
            .ok_or_else(invalid)?;
        if first == signature {
            matching.push(index);
        }
    }
    if matching.len() != 1 {
        return Err(invalid());
    }
    let index = matching[0];
    let source = Digest::of(raw);
    let features: Vec<Value> = binding["features"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|feature| feature["id"].clone())
        .collect();
    Ok(
        json!({"id":format!("tx-{signature}"),"slot":slot,"transactionIndex":index,"targetSignature":signature,
        "blockSourceHash":source,"rawEvidenceHash":Digest::of(format!("{}\n{index}\n{signature}\n", source.as_str())),
        "runtimeProfileId":binding["runtimeProfileId"],"executorSourceId":binding["executor"]["id"],
        "litesvmCommit":binding["executor"]["litesvmCommit"],"activeFeatures":features}),
    )
}
