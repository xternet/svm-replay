use super::*;

const MEMO_PROGRAMS: [&str; 2] = [
    "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo",
    "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",
];

/// Archived data that does not have its documented shape: refused, never repaired.
pub(in super::super) fn integrity(message: &str) -> Error {
    Error::new("SOURCE_INTEGRITY", message)
}

/// An archived produced-slot list: strictly ascending and within `start..=end`.
pub(in super::super) fn ascending(slots: &Value, start: u64, end: u64) -> Result<Vec<u64>, Error> {
    let list = slots
        .as_array()
        .ok_or_else(|| integrity("archived block list missing"))?;
    let mut checked: Vec<u64> = Vec::with_capacity(list.len());
    for at in list {
        match at.as_u64().filter(|at| (start..=end).contains(at)) {
            Some(at) if checked.last().is_none_or(|last| *last < at) => checked.push(at),
            _ => {
                return Err(integrity(
                    "archived block list is not ascending within its range",
                ))
            }
        }
    }
    Ok(checked)
}

pub(in super::super) fn transactions(block: &Value) -> Result<&Vec<Value>, Error> {
    block["transactions"]
        .as_array()
        .ok_or_else(|| integrity("archived block transactions missing"))
}

pub(in super::super) fn base58(value: &Value, len: usize, what: &str) -> Result<String, Error> {
    let text = value.as_str().ok_or_else(|| integrity(what))?;
    match bs58::decode(text).into_vec() {
        Ok(bytes) if bytes.len() == len => Ok(text.to_owned()),
        _ => Err(integrity(what)),
    }
}

pub(in super::super) fn signature(entry: &Value) -> Result<String, Error> {
    base58(
        &entry["transaction"]["signatures"][0],
        64,
        "archived transaction signature",
    )
}

/// An archived transaction as the fork knows it: its exact wire bytes (SVM Replay's codec of
/// the archived JSON) beside the archived metadata, version, slot, block time and its index
/// in its block when known.
pub(in super::super) fn record(
    entry: &Value,
    slot: u64,
    block_time: &Value,
    index: Option<u64>,
) -> Result<Value, Error> {
    if !entry["meta"].is_object() {
        return Err(integrity("archived transaction metadata missing"));
    }
    Ok(
        json!({"slot":slot,"blockTime":block_time,"version":entry["version"],"meta":entry["meta"],
        "transactionIndex":index,"transactionBase64":svm_replay_protocol::transaction::encode(entry)?}),
    )
}

pub(in super::super) fn records(
    block: &Value,
    slot: u64,
    count: usize,
) -> Result<Vec<Value>, Error> {
    transactions(block)?[..count]
        .iter()
        .enumerate()
        .map(|(index, entry)| record(entry, slot, &block["blockTime"], Some(index as u64)))
        .collect()
}

/// Every key a transaction's address index holds: its static keys and the addresses its
/// lookup tables loaded.
pub(in super::super) fn keys(entry: &Value) -> Result<Vec<String>, Error> {
    let loaded = &entry["meta"]["loadedAddresses"];
    let mut lists = vec![&entry["transaction"]["message"]["accountKeys"]];
    if !loaded.is_null() {
        lists.extend([&loaded["writable"], &loaded["readonly"]]);
    }
    let mut keys = Vec::new();
    for list in lists {
        for key in list
            .as_array()
            .ok_or_else(|| integrity("archived account keys missing"))?
        {
            keys.push(base58(key, 32, "archived account key")?);
        }
    }
    Ok(keys)
}

/// The memo the RPC index reports for a transaction, as Agave formats it.
pub(in super::super) fn memo(entry: &Value) -> Result<Value, Error> {
    let message = &entry["transaction"]["message"];
    let static_keys = message["accountKeys"]
        .as_array()
        .ok_or_else(|| integrity("archived account keys missing"))?;
    let instructions = message["instructions"]
        .as_array()
        .ok_or_else(|| integrity("archived instructions missing"))?;
    let mut memos = Vec::new();
    for instruction in instructions {
        let program = instruction["programIdIndex"]
            .as_u64()
            .and_then(|index| static_keys.get(index as usize))
            .ok_or_else(|| integrity("archived program index"))?;
        if MEMO_PROGRAMS.iter().any(|memo| program == memo) {
            let data = bs58::decode(
                instruction["data"]
                    .as_str()
                    .ok_or_else(|| integrity("archived memo data"))?,
            )
            .into_vec()
            .map_err(|_| integrity("archived memo data is not base58"))?;
            let text = std::str::from_utf8(&data).unwrap_or("(unparseable)");
            memos.push(format!("[{}] {text}", data.len()));
        }
    }
    Ok(if memos.is_empty() {
        Value::Null
    } else {
        json!(memos.join("; "))
    })
}
