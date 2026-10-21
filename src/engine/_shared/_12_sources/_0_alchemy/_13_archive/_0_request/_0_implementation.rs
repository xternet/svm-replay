use super::*;

/// One finalized history read: a transaction by signature, an address's signatures, the
/// produced blocks of a slot range, a block's header, an owner's token accounts or a mint's
/// holders at a slot, or a page of an address's history. Every field is validated before it
/// reaches the provider.
pub(in super::super) fn request(query: &Value) -> Result<Value> {
    let limit = |value: &Value, max: u64| {
        value
            .as_u64()
            .filter(|n| (1..=max).contains(n))
            .ok_or_else(|| SourceError::new("INVALID_QUERY", "archive limit out of range"))
    };
    match query["kind"].as_str() {
        Some("transaction") => {
            address(
                &query["signature"],
                64,
                "INVALID_QUERY",
                "archive signature",
            )?;
            Ok(
                json!({"jsonrpc":"2.0","id":1,"method":"getTransaction","params":[query["signature"],
                {"encoding":"json","commitment":"finalized","maxSupportedTransactionVersion":1}]}),
            )
        }
        Some("signatures") => {
            address(&query["address"], 32, "INVALID_QUERY", "archive address")?;
            let mut config =
                json!({"commitment":"finalized","limit":limit(&query["limit"], 1000)?});
            for cursor in ["before", "until"] {
                if !query[cursor].is_null() {
                    address(
                        &query[cursor],
                        64,
                        "INVALID_QUERY",
                        "archive signature cursor",
                    )?;
                    config[cursor] = query[cursor].clone();
                }
            }
            Ok(
                json!({"jsonrpc":"2.0","id":1,"method":"getSignaturesForAddress","params":[query["address"],config]}),
            )
        }
        // A produced block's header alone: its blockhash, parent and previous blockhash.
        Some("header") => {
            let slot = slot(&query["slot"], "INVALID_QUERY", "archive header slot")?;
            Ok(
                json!({"jsonrpc":"2.0","id":1,"method":"getBlock","params":[slot,{"encoding":"json",
                "transactionDetails":"none","rewards":false,"commitment":"finalized",
                "maxSupportedTransactionVersion":1}]}),
            )
        }
        Some("blocks") => {
            let start = slot(&query["start"], "INVALID_QUERY", "archive start slot")?;
            let end = slot(&query["end"], "INVALID_QUERY", "archive end slot")?;
            require(
                start <= end && end - start <= 500_000,
                "INVALID_QUERY",
                "archive slot range",
            )?;
            Ok(
                json!({"jsonrpc":"2.0","id":1,"method":"getBlocks","params":[start,end,{"commitment":"finalized"}]}),
            )
        }
        Some("tokenAccountsByOwner") => token_accounts(query),
        // A mint's nonzero token accounts at the end of `slot`, largest first, at most `limit`.
        Some("tokenHolders") => {
            address(&query["mint"], 32, "INVALID_QUERY", "archive holder mint")?;
            let slot = slot(&query["slot"], "INVALID_QUERY", "archive holder slot")?;
            Ok(
                json!({"jsonrpc":"2.0","id":1,"method":"getTokenHoldersAtSlot","params":[query["mint"],
                slot,limit(&query["limit"], 10_000)?,"balance_desc"]}),
            )
        }
        Some("transactionsForAddress") => address_history(query, limit(&query["limit"], 1000)?),
        // An account's image at the end of `slot`, as an end-slot account read requests it.
        Some("accountAtSlot") => {
            address(&query["pubkey"], 32, "INVALID_QUERY", "archive account")?;
            let slot = slot(&query["slot"], "INVALID_QUERY", "archive account slot")?;
            Ok(
                json!({"jsonrpc":"2.0","id":1,"method":"getAccountInfo","params":[query["pubkey"],
                {"encoding":"base64","commitment":"finalized","slot":slot}]}),
            )
        }
        _ => Err(SourceError::new("INVALID_QUERY", "unknown archive query")),
    }
}

/// One page of the token accounts an owner held at the end of `slot` (always jsonParsed),
/// filtered by exactly one of mint or token program.
fn token_accounts(query: &Value) -> Result<Value> {
    let invalid = |label: &str| SourceError::new("INVALID_QUERY", label);
    address(&query["owner"], 32, "INVALID_QUERY", "archive token owner")?;
    let filter = match (query["mint"].is_null(), query["programId"].is_null()) {
        (false, true) => {
            address(&query["mint"], 32, "INVALID_QUERY", "archive token mint")?;
            json!({"mint":query["mint"]})
        }
        (true, false) => {
            address(
                &query["programId"],
                32,
                "INVALID_QUERY",
                "archive token program",
            )?;
            json!({"programId":query["programId"]})
        }
        _ => {
            return Err(invalid(
                "archive token filter needs exactly one of mint or programId",
            ))
        }
    };
    let slot = slot(&query["slot"], "INVALID_QUERY", "archive token slot")?;
    let mut config = json!({"slot":slot,"pageLimit":10_000,"encoding":"jsonParsed"});
    if !query["pageKey"].is_null() {
        let key = query["pageKey"]
            .as_str()
            .filter(|key| !key.is_empty() && key.len() <= 1024)
            .ok_or_else(|| invalid("archive token page key"))?;
        config["pageKey"] = json!(key);
    }
    Ok(
        json!({"jsonrpc":"2.0","id":1,"method":"getTokenAccountsByOwnerAtSlot","params":[query["owner"],filter,config]}),
    )
}

/// One page of an address's finalized history up to `untilSlot`, in either order, with the
/// provider's own status and token-account filters and its `slot:position` page token.
fn address_history(query: &Value, limit: u64) -> Result<Value> {
    let invalid = |label: &str| SourceError::new("INVALID_QUERY", label);
    address(
        &query["address"],
        32,
        "INVALID_QUERY",
        "archive history address",
    )?;
    let one_of = |field: &str, allowed: &[&str]| {
        query[field]
            .as_str()
            .filter(|value| allowed.contains(value))
            .ok_or_else(|| invalid("archive history option"))
    };
    let until = slot(&query["untilSlot"], "INVALID_QUERY", "archive history slot")?;
    // A caller's comparison filter: some of `ops`, each with a slot or a signed time bound.
    let comparison = |field: &str, ops: &[&str], signed: bool| match &query[field] {
        Value::Null => Ok(None),
        Value::Object(bounds)
            if !bounds.is_empty()
                && bounds.iter().all(|(op, bound)| {
                    ops.contains(&op.as_str())
                        && match signed {
                            true => bound.is_i64(),
                            false => bound.as_u64().is_some_and(|b| b <= 9_007_199_254_740_991),
                        }
                }) =>
        {
            Ok(Some(bounds.clone()))
        }
        _ => Err(invalid("archive history filter")),
    };
    // The fork's history ends at the boundary slot, whatever the caller's own slot bounds.
    let mut slots = comparison("slot", &["gte", "gt", "lte", "lt"], false)?.unwrap_or_default();
    let lte = slots
        .get("lte")
        .and_then(Value::as_u64)
        .map_or(until, |lte| lte.min(until));
    slots.insert("lte".into(), json!(lte));
    let mut filters = json!({"slot":slots,
        "tokenAccounts":one_of("tokenAccounts", &["none","balanceChanged","all"])?,
        "status":one_of("status", &["any","succeeded","failed"])?});
    if let Some(times) = comparison("blockTime", &["gte", "gt", "lte", "lt", "eq"], true)? {
        filters["blockTime"] = Value::Object(times);
    }
    let mut config = json!({"transactionDetails":"signatures","sortOrder":one_of("sortOrder", &["asc","desc"])?,
        "limit":limit,"commitment":"finalized","filters":filters});
    if !query["paginationToken"].is_null() {
        let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
        let token = query["paginationToken"]
            .as_str()
            .filter(|token| {
                token
                    .split_once(':')
                    .is_some_and(|(slot, index)| digits(slot) && digits(index))
            })
            .ok_or_else(|| invalid("archive history page token"))?;
        config["paginationToken"] = json!(token);
    }
    Ok(
        json!({"jsonrpc":"2.0","id":1,"method":"getTransactionsForAddress","params":[query["address"],config]}),
    )
}
