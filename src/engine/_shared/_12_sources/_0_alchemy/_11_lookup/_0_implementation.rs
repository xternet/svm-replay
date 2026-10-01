use super::*;

pub(in super::super) fn validate_shape(query: &Value, request: &Value, raw: &[u8]) -> Result<()> {
    if query["kind"] == "transaction" {
        require(
            request.is_object() == parse(raw)?.is_object(),
            "SOURCE_INTEGRITY",
            "transaction response does not match its RPC endpoint",
        )?;
    }
    Ok(())
}

pub(in super::super) fn missing(raw: &[u8], batch: bool) -> Result<bool> {
    let value = parse(raw)?;
    let response = if batch {
        let rows = value
            .as_array()
            .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "transaction batch missing"))?;
        require(
            rows.len() == 1,
            "SOURCE_INTEGRITY",
            "transaction batch count differs",
        )?;
        &rows[0]
    } else {
        &value
    };
    Ok(envelope(response, if batch { 0 } else { 1 })?.is_null())
}

pub(in super::super) fn status_request(signature: &str) -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"getSignatureStatuses","params":[[signature],{"searchTransactionHistory":true}]})
}

pub(in super::super) fn status_slot(raw: &[u8]) -> Result<u64> {
    let value = parse(raw)?;
    let result = envelope(&value, 1)?;
    let values = result["value"]
        .as_array()
        .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "signature statuses missing"))?;
    require(
        values.len() == 1,
        "SOURCE_INTEGRITY",
        "signature status count differs",
    )?;
    let status = &values[0];
    require(
        status.is_object(),
        "SOURCE_UNAVAILABLE",
        "historical signature status unavailable",
    )?;
    require(
        status["confirmationStatus"] == "finalized",
        "SOURCE_UNAVAILABLE",
        "signature is not finalized",
    )?;
    let found = slot(&status["slot"], "SOURCE_INTEGRITY", "signature status slot")?;
    require(
        slot(
            &result["context"]["slot"],
            "SOURCE_INTEGRITY",
            "status context slot",
        )? >= found,
        "SOURCE_CONTEXT_MISMATCH",
        "status slot exceeds observation context",
    )?;
    Ok(found)
}

pub(in super::super) fn validate_location(value: &Value, signature: &str) -> Result<u64> {
    require(
        value["lookupKind"] == "signature-status",
        "SOURCE_INTEGRITY",
        "unknown signature lookup kind",
    )?;
    require(
        value["statusRequest"] == status_request(signature),
        "SOURCE_INTEGRITY",
        "status lookup request differs",
    )?;
    let original = canonical_base64(&value["responseBodyBase64"], "original lookup bytes")?;
    require(
        missing(&original, false)?,
        "SOURCE_INTEGRITY",
        "status recovery requires original null",
    )?;
    let raw = canonical_base64(&value["statusResponseBodyBase64"], "status lookup bytes")?;
    require(
        value["statusResponseSha256"] == json!(Digest::of(&raw)),
        "SOURCE_INTEGRITY",
        "status lookup digest differs",
    )?;
    status_slot(&raw)
}

pub(in super::super) fn block_request(query: &Value) -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"getBlock","params":[query["slot"],{"encoding":"base64","commitment":"finalized","transactionDetails":"full","rewards":true,"maxSupportedTransactionVersion":1}]})
}

pub(in super::super) fn block_wire(query: &Value, payload: &Value) -> Result<Value> {
    let block = envelope(payload, 1)?;
    require(
        block.is_object(),
        "SOURCE_UNAVAILABLE",
        "historical transaction block unavailable",
    )?;
    let target = slot(&query["slot"], "INVALID_QUERY", "transaction slot")?;
    require(
        slot(
            &block["parentSlot"],
            "SOURCE_INTEGRITY",
            "transaction block parent",
        )? < target,
        "SOURCE_CONTEXT_MISMATCH",
        "transaction block parent invalid",
    )?;
    let rows = block["transactions"]
        .as_array()
        .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "block transactions missing"))?;
    let mut found = None;
    for row in rows {
        let wire = row["transaction"].as_array().ok_or_else(|| {
            SourceError::new("SOURCE_INTEGRITY", "block transaction tuple missing")
        })?;
        require(
            wire.len() == 2 && wire[1] == "base64",
            "SOURCE_INTEGRITY",
            "block transaction encoding differs",
        )?;
        let encoded = wire[0].as_str().ok_or_else(|| {
            SourceError::new("SOURCE_INTEGRITY", "block transaction bytes missing")
        })?;
        let decoded = svm_replay_protocol::transaction::decode(encoded).map_err(|e| {
            SourceError::new("SOURCE_INTEGRITY", format!("block wire: {}", e.message))
        })?;
        if decoded["transaction"]["signatures"][0] == query["signature"] {
            require(
                found.is_none(),
                "SOURCE_INTEGRITY",
                "duplicate signature in historical block",
            )?;
            found = Some(wire[0].clone());
        }
    }
    found.ok_or_else(|| {
        SourceError::new(
            "SOURCE_UNAVAILABLE",
            "signature absent from historical block",
        )
    })
}
