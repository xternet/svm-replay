use super::*;

pub(super) const GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";

pub(super) const RENT: &str = "SysvarRent111111111111111111111111111111111";

pub(super) type Result<T> = std::result::Result<T, SourceError>;

pub(super) fn invalid(message: impl Into<String>) -> SourceError {
    SourceError::new("RENT_CAPTURE_SCOPE", message)
}

pub(super) fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(invalid(message))
    }
}

pub(super) fn checked_query(q: &Value) -> Result<u64> {
    let slot = q["slot"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
        .ok_or_else(|| invalid("exact safe target slot required"))?;
    require(
        q.as_object().is_some_and(|o| o.len() == 5) && *q == query(slot),
        "only mainnet/end-slot/exact Rent queries are authorized",
    )?;
    Ok(slot)
}

pub(super) fn validate_queries(queries: &[Value]) -> Result<()> {
    require(
        queries.len() == 16,
        "this corpus capture requires exactly16 unique queries",
    )?;
    let mut previous = None;
    for q in queries {
        let slot = checked_query(q)?;
        require(
            previous.is_none_or(|before| before < slot),
            "queries must be unique and slot-sorted",
        )?;
        previous = Some(slot);
    }
    Ok(())
}

pub(super) fn freeze(summary: &Value, summary_sha256: &Digest) -> Result<Value> {
    require(
        summary["schema"] == "svm-replay-m17-guarded-prepared-parity/v1"
            && summary["status"] == "INCOMPLETE"
            && summary["count"] == 47
            && summary["completed"] == 30
            && summary["networkRequests"] == 0
            && summary["reconstructedFromSources"] == false,
        "not the reviewed retained-only corpus shape",
    )?;
    let results = summary["results"]
        .as_array()
        .ok_or_else(|| invalid("summary results missing"))?;
    require(results.len() == 47, "summary result count differs")?;
    let mut queries = std::collections::BTreeMap::new();
    let mut cases = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for row in results {
        let case = row["caseId"]
            .as_str()
            .ok_or_else(|| invalid("case id missing"))?;
        require(seen.insert(case), "duplicate case id")?;
        if row["outcome"] == "COMPLETED" {
            require(
                row["code"] == 0 && row["error"].is_null(),
                "completed case contains failure",
            )?;
            continue;
        }
        require(
            row["outcome"] == "UNSUPPORTED"
                && row["code"] == 1
                && row["error"]["code"] == "SOURCE_UNAVAILABLE",
            "non-source failure needs separate review",
        )?;
        let details = &row["error"]["details"];
        require(
            details["discovery"]["schema"] == "svm-sysvar-discovery/v1"
                && details["discovery"]["status"] == "NEEDS_INPUT"
                && details["discovery"]["pubkey"] == RENT
                && details["sourceError"]["code"] == "SOURCE_UNAVAILABLE",
            "missing exact Rent discovery evidence",
        )?;
        let q = &details["sourceError"]["details"]["query"];
        let slot = checked_query(q)?;
        queries.insert(slot, q.clone());
        cases.push(json!({"caseId":case,"query":q}));
    }
    require(cases.len() == 17, "reviewed missing case count differs")?;
    let queries = queries.into_values().collect::<Vec<_>>();
    validate_queries(&queries)?;
    Ok(
        json!({"schema":"svm-replay-m17-exact-rent-queries/v1","summarySha256":summary_sha256,"queries":queries,"cases":cases}),
    )
}

pub(super) fn validate_frozen(frozen: &Value) -> Result<Vec<Value>> {
    require(
        frozen["schema"] == "svm-replay-m17-exact-rent-queries/v1",
        "frozen query schema differs",
    )?;
    Digest::new(
        frozen["summarySha256"]
            .as_str()
            .ok_or_else(|| invalid("summary pin missing"))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    let queries = frozen["queries"]
        .as_array()
        .ok_or_else(|| invalid("frozen queries missing"))?
        .clone();
    validate_queries(&queries)?;
    Ok(queries)
}

pub(super) fn limits() -> SourceLimits {
    SourceLimits {
        max_requests: 17,
        max_account_reads: 16,
        max_download_bytes: 2 * 1024 * 1024,
        max_response_bytes: 64 * 1024,
        deadline: Duration::from_secs(120),
    }
}

pub(super) fn config(queries: &[Value]) -> AlchemyConfig {
    AlchemyConfig {
        id: "m17-exact-target-rent".into(),
        version: "v1".into(),
        expected_genesis_hash: GENESIS.into(),
        first_slot: queries[0]["slot"].as_u64().expect("validated query"),
        last_slot: queries[15]["slot"].as_u64().expect("validated query"),
    }
}

pub(super) fn io(error: std::io::Error) -> SourceError {
    SourceError::new("RENT_CAPTURE_IO", error.to_string())
}

pub(super) fn write_new(path: &Path, value: &Value) -> Result<Digest> {
    use std::io::Write;
    let bytes = serde_json::to_vec(value).map_err(|e| invalid(e.to_string()))?;
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(io)?;
    file.write_all(&bytes).map_err(io)?;
    file.sync_all().map_err(io)?;
    Ok(Digest::of(bytes))
}

pub(super) fn create_private(path: &Path) -> Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(io)
}
