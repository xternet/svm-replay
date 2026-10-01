use super::*;

pub(in super::super::super) fn value_from_response(query: &Value, raw: &[u8]) -> Result<Value> {
    let payload = parse(raw)?;
    if query["kind"] == "transaction" && payload.is_object() {
        return _11_lookup::block_wire(query, &payload);
    }
    if query["kind"] == "block" {
        let result = envelope(&payload, 1)?;
        require(
            result.is_object(),
            "SOURCE_UNAVAILABLE",
            "historical block unavailable",
        )?;
        return Ok(json!({"rawBase64":STANDARD.encode(raw)}));
    }
    let batch = payload
        .as_array()
        .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "historical RPC batch required"))?;
    require(
        batch.len() == 1,
        "SOURCE_INTEGRITY",
        "historical RPC response count/id mismatch",
    )?;
    let result = envelope(&batch[0], 0)?;
    if query["kind"] == "account" {
        require(
            result.is_object(),
            "SOURCE_INTEGRITY",
            "account result missing",
        )?;
        let observed = slot(
            &result["context"]["slot"],
            "SOURCE_INTEGRITY",
            "account context slot",
        )?;
        let target = slot(&query["slot"], "INVALID_QUERY", "slot")?;
        require(
            if query["phase"] == "last-write-at-or-before-slot" {
                observed <= target
            } else {
                observed == target
            },
            "SOURCE_CONTEXT_MISMATCH",
            "historical account context slot differs from request",
        )?;
        let value = result
            .get("value")
            .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "account value missing"))?;
        let base = json!({"pubkey":query["pubkey"],"sourceSlot":observed,"role":"application","presence":if value.is_null(){"absent"}else{"present"}});
        if value.is_null() {
            return Ok(base);
        }
        require(
            value.is_object(),
            "SOURCE_INTEGRITY",
            "account value malformed",
        )?;
        address(&value["owner"], 32, "SOURCE_INTEGRITY", "account owner")?;
        require(
            value["executable"].is_boolean(),
            "SOURCE_INTEGRITY",
            "account executable malformed",
        )?;
        let data = value["data"]
            .as_array()
            .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "account data tuple missing"))?;
        require(
            data.len() == 2 && data[1] == "base64",
            "SOURCE_INTEGRITY",
            "account data encoding differs",
        )?;
        canonical_base64(&data[0], "account base64 malformed")?;
        let mut account = base;
        account["owner"] = value["owner"].clone();
        account["executable"] = value["executable"].clone();
        account["lamports"] = json!(decimal(&value["lamports"], "lamports")?);
        account["rentEpoch"] = json!(decimal(&value["rentEpoch"], "rentEpoch")?);
        account["dataBase64"] = data[0].clone();
        Ok(account)
    } else {
        require(
            result.is_object(),
            "SOURCE_UNAVAILABLE",
            "historical transaction unavailable",
        )?;
        require(
            result["slot"] == query["slot"],
            "SOURCE_CONTEXT_MISMATCH",
            "historical transaction slot differs from request",
        )?;
        let wire = result["transaction"].as_array().ok_or_else(|| {
            SourceError::new("SOURCE_INTEGRITY", "transaction base64 tuple missing")
        })?;
        require(
            wire.len() == 2 && wire[1] == "base64" && wire[0].is_string(),
            "SOURCE_INTEGRITY",
            "transaction encoding differs",
        )?;
        Ok(wire[0].clone())
    }
}
