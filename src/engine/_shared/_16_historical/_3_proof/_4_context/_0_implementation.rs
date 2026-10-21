use super::*;

pub(in super::super) fn build(
    block: &Value,
    source_hash: &Digest,
    fixture: &Value,
    after: bool,
    capabilities: &Value,
) -> Result<Value, Error> {
    check(
        fixture["runtime"]["bankContext"]["blockSourceHash"] == source_hash.as_str(),
        "RPC context block differs from the prepared source",
    )?;
    let transactions = block["transactions"]
        .as_array()
        .ok_or_else(|| invalid("complete archived block transactions required"))?;
    let records = transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| {
            let error = transaction["meta"]
                .get("err")
                .ok_or_else(|| invalid("archived transaction status missing"))?;
            let mut record = json!({"index":index,"transactionBase64":svm_replay_protocol::transaction::encode(transaction)?,"err":error});
            if capabilities.as_array().is_some_and(|caps| caps.iter().any(|cap| cap == "native-fee-accounting/v1")) {
                let (fee, loaded) = fee_evidence(transaction)?;
                record["fee"] = json!(fee);
                record["loadedAddresses"] = loaded;
            }
            Ok(record)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let position = integer(&fixture["target"]["index"])?
        .checked_add(u64::from(after))
        .ok_or_else(|| invalid("session transaction position overflow"))?;
    Ok(
        json!({"schema":"svm-native-rpc-context/v1","sourceHash":source_hash,
        "slot":fixture["target"]["targetSlot"],"transactionIndex":position,
        "blockHeight":integer(&block["blockHeight"])?,"transactions":records}),
    )
}

pub(super) fn fee_evidence(transaction: &Value) -> Result<(u64, Value), Error> {
    let fee = transaction["meta"]["fee"]
        .as_u64()
        .ok_or_else(|| invalid("archived fee missing or invalid"))?;
    let loaded = match transaction["meta"].get("loadedAddresses") {
        Some(value) if !value.is_null() => value.clone(),
        _ => {
            if transaction["transaction"]["message"]["addressTableLookups"]
                .as_array()
                .is_some_and(|tables| !tables.is_empty())
            {
                return Err(invalid("archived loaded addresses missing"));
            }
            json!({"writable":[],"readonly":[]})
        }
    };
    Ok((fee, loaded))
}
