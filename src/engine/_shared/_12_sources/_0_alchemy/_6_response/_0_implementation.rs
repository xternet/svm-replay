use super::*;

pub(in super::super) fn validate_limits(limits: &SourceLimits) -> Result<()> {
    require(
        limits.max_requests > 0
            && limits.max_account_reads > 0
            && limits.max_download_bytes > 0
            && limits.max_response_bytes > 0
            && limits.max_response_bytes <= 64 * 1024 * 1024
            && limits.max_download_bytes <= 9_007_199_254_740_991
            && limits.max_requests <= 9_007_199_254_740_991
            && limits.max_account_reads <= 9_007_199_254_740_991
            && !limits.deadline.is_zero(),
        "SOURCE_CONFIGURATION",
        "invalid source limits",
    )
}

pub(in super::super::super) fn request_for_query(query: &Value) -> Result<Value> {
    Ok(match query["kind"].as_str() {
        Some("account") => {
            let mut options = json!({"commitment":"finalized","encoding":"base64"});
            if query["phase"] == "last-write-at-or-before-slot" {
                let target = slot(&query["slot"], "INVALID_QUERY", "archive cursor")?;
                require(
                    target < 9_007_199_254_740_991,
                    "INVALID_QUERY",
                    "archive cursor overflow",
                )?;
                options["lastUpdateBeforeSlot"] = json!(target + 1);
            } else {
                options["slot"] = query["slot"].clone();
            }
            json!([{"jsonrpc":"2.0","id":0,"method":"getAccountInfo","params":[query["pubkey"],options]}])
        }
        Some("transaction") => {
            json!([{"jsonrpc":"2.0","id":0,"method":"getTransaction","params":[query["signature"],{"encoding":"base64","commitment":"finalized","maxSupportedTransactionVersion":1}]}])
        }
        Some("block") => {
            json!({"jsonrpc":"2.0","id":1,"method":"getBlock","params":[query["slot"],{"encoding":"json","commitment":"finalized","transactionDetails":"full","rewards":true,"maxSupportedTransactionVersion":1}]})
        }
        _ => {
            return Err(SourceError::new(
                "INVALID_QUERY",
                "RPC capability unavailable",
            ))
        }
    })
}

pub(in super::super) fn request_matches(query: &Value, request: &Value) -> Result<bool> {
    if query["kind"] == "transaction" && request.is_object() {
        return Ok(*request == _11_lookup::block_request(query));
    }
    let expected = request_for_query(query)?;
    let mut normalized = request.clone();
    let options = match query["kind"].as_str() {
        Some("block") => normalized.pointer_mut("/params/1"),
        Some("transaction") => normalized.pointer_mut("/0/params/1"),
        _ => return Ok(normalized == expected),
    };
    let Some(options) = options else {
        return Ok(false);
    };
    if !matches!(
        options["maxSupportedTransactionVersion"].as_u64(),
        Some(0 | 1)
    ) {
        return Ok(false);
    }
    options["maxSupportedTransactionVersion"] = json!(1);
    if query["kind"] == "block" && options["rewards"] == false {
        options["rewards"] = json!(true);
    }
    Ok(normalized == expected)
}

pub(in super::super) fn parse(raw: &[u8]) -> Result<Value> {
    parse_json(raw)
        .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "malformed or duplicate-key RPC JSON"))
}

pub(in super::super) fn envelope(value: &Value, id: u64) -> Result<&Value> {
    require(
        value.is_object() && value["jsonrpc"] == "2.0" && value["id"].as_u64() == Some(id),
        "SOURCE_INTEGRITY",
        "RPC response envelope/id mismatch",
    )?;
    require(
        value.get("result").is_some() != value.get("error").is_some(),
        "SOURCE_INTEGRITY",
        "RPC response must contain exactly result or error",
    )?;
    if let Some(error) = value.get("error") {
        let code = error["code"]
            .as_i64()
            .filter(|code| (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(code))
            .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "malformed RPC error code"))?;
        return Err(SourceError::new(
            if code == 429 {
                "SOURCE_RATE_LIMIT"
            } else if code == -32015 {
                "UNSUPPORTED_TRANSACTION_VERSION"
            } else {
                "SOURCE_RPC_ERROR"
            },
            "historical RPC error",
        )
        .details(json!({"rpcCode":code})));
    }
    Ok(&value["result"])
}

pub(in super::super) fn checked_genesis(raw: &[u8], expected: &Value) -> Result<()> {
    let value = parse(raw)?;
    let result = envelope(&value, 1)?;
    address(result, 32, "SOURCE_INTEGRITY", "RPC genesis hash")?;
    require(
        result == expected,
        "SOURCE_CONTEXT_MISMATCH",
        "historical RPC network differs from configured genesis",
    )
}

pub(in super::super) fn record_from_response(
    query: &Value,
    request: &Value,
    raw: &[u8],
    genesis: &[u8],
) -> Result<Value> {
    let value = value_from_response(query, raw)?;
    let request_hash = Digest::of(
        serde_json::to_vec(request)
            .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "RPC request serialization"))?,
    );
    let response_hash = Digest::of(raw);
    let genesis_hash = Digest::of(genesis);
    let record = json!({"query":query,"value":value,"evidenceHashes":[request_hash,response_hash,genesis_hash],"provenance":{"schema":"m17-alchemy-rpc-provenance/v1","request":request,"requestSha256":request_hash,"responseSha256":response_hash,"responseBytes":raw.len(),"responseBodyBase64":STANDARD.encode(raw),"genesisResponseSha256":genesis_hash,"genesisResponseBodyBase64":STANDARD.encode(genesis)}});
    validate_record(&record, query)?;
    Ok(record)
}

pub(in super::super) fn decimal(value: &Value, field: &str) -> Result<String> {
    let text = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => {
            return Err(SourceError::new(
                "SOURCE_INTEGRITY",
                format!("account {field} must be exact u64"),
            ))
        }
    };
    let number = text.parse::<u64>().map_err(|_| {
        SourceError::new(
            "SOURCE_INTEGRITY",
            format!("account {field} must be exact u64"),
        )
    })?;
    require(
        number.to_string() == text,
        "SOURCE_INTEGRITY",
        &format!("account {field} must be canonical u64"),
    )?;
    Ok(text)
}
