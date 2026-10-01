use super::*;

pub(in super::super) fn validate_query(q: &Value) -> Result<()> {
    let mut fields = vec!["kind", "genesisHash", "slot"];
    match q["kind"].as_str() {
        Some("block") => fields.push("blockEvidenceSha256"),
        Some("account") => fields.extend(["pubkey", "phase"]),
        Some("transaction") => fields.push("signature"),
        Some("bank-input") => fields.extend([
            "parentSlot",
            "blockhash",
            "blockEvidenceSha256",
            "executorSourceId",
            "runtimeProfileId",
            "activeFeatureSetHash",
            "phase",
            "input",
        ]),
        _ => return Err(SourceError::new("INVALID_QUERY", "unknown capability")),
    }
    object(q, &fields, "INVALID_QUERY", "query")?;
    address(&q["genesisHash"], 32, "INVALID_QUERY", "genesis")?;
    let target_slot = slot(&q["slot"], "INVALID_QUERY", "slot")?;
    if q["kind"] == "block" || q["kind"] == "bank-input" {
        hash(
            &q["blockEvidenceSha256"],
            "INVALID_QUERY",
            "block evidence hash",
        )?;
    }
    if q["kind"] == "account" {
        address(&q["pubkey"], 32, "INVALID_QUERY", "account pubkey")?;
        require(
            q["phase"] == "end-slot" || q["phase"] == "last-write-at-or-before-slot",
            "INVALID_QUERY",
            "account phase",
        )?;
        if q["phase"] == "last-write-at-or-before-slot" {
            require(
                target_slot < 9_007_199_254_740_991,
                "INVALID_QUERY",
                "archive cursor overflow",
            )?;
        }
    }
    if q["kind"] == "transaction" {
        address(
            &q["signature"],
            64,
            "INVALID_QUERY",
            "transaction signature",
        )?;
    }
    if q["kind"] == "bank-input" {
        require(
            slot(&q["parentSlot"], "INVALID_QUERY", "parent slot")? < target_slot,
            "INVALID_QUERY",
            "parent order",
        )?;
        address(&q["blockhash"], 32, "INVALID_QUERY", "blockhash")?;
        hash(&q["activeFeatureSetHash"], "INVALID_QUERY", "feature hash")?;
        public_id(&q["executorSourceId"], "INVALID_QUERY", "executor identity")?;
        public_id(&q["runtimeProfileId"], "INVALID_QUERY", "runtime profile")?;
        require(
            q["phase"] == "post-bank-initialization/pre-transaction",
            "INVALID_QUERY",
            "Bank phase",
        )?;
        require(
            [
                "initializedStakeEvidence",
                "epochStakeEvidence",
                "initializationEvidence",
                "programMigrationEvidence",
            ]
            .iter()
            .any(|kind| q["input"] == *kind),
            "INVALID_QUERY",
            "Bank input kind",
        )?;
    }
    Ok(())
}

pub(in super::super) fn value_hash(domain: &str, value: &Value) -> Result<Digest> {
    // serde_json::Map is recursively key-sorted in this workspace (no preserve_order).
    let encoded = serde_json::to_string(value)
        .map_err(|e| SourceError::new("SOURCE_INTEGRITY", e.to_string()))?;
    Ok(Digest::of(format!("{domain}\n{encoded}\n")))
}

pub fn query_key(query: &Value) -> Result<Digest> {
    json_value(query)?;
    validate_query(query)?;
    value_hash("m11-source-query/v1", query)
}
