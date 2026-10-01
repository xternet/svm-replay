use super::*;

pub(in super::super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("FEE_EVIDENCE", message)
}

pub(in super::super) fn amount(value: &Value) -> Result<u64, Error> {
    if let Some(number) = value.as_u64() {
        return Ok(number);
    }
    if let Some(text) = value.as_str() {
        let n = text
            .parse::<u64>()
            .map_err(|_| invalid("balance/fee outside u64"))?;
        if n.to_string() == text {
            return Ok(n);
        }
    }
    Err(invalid("exact unsigned balance/fee required"))
}

pub(in super::super) fn amounts(value: &Value) -> Result<Vec<u64>, Error> {
    value
        .as_array()
        .ok_or_else(|| invalid("balance array missing"))?
        .iter()
        .map(amount)
        .collect()
}

pub fn omitted_fee_effects_hash(effects: &[Value]) -> Result<Digest, Error> {
    let mut lines = Vec::new();
    for effect in effects {
        for field in [
            "index",
            "signature",
            "feePayer",
            "feeLamports",
            "preLamports",
            "postLamports",
        ] {
            let value = effect
                .get(field)
                .ok_or_else(|| invalid(format!("effect missing {field}")))?;
            lines.push(if field == "index" {
                value
                    .as_u64()
                    .ok_or_else(|| invalid("effect index"))?
                    .to_string()
            } else {
                value
                    .as_str()
                    .ok_or_else(|| invalid("effect field must be string"))?
                    .into()
            });
        }
    }
    Ok(Digest::of(format!(
        "svm-call-omitted-fee-effects-v1\n{}\n",
        lines.join("\n")
    )))
}

pub fn derive_omitted_fee_effects(
    raw: &[Value],
    transactions: &[SemanticTransaction],
    target_index: u64,
    closure: &SemanticClosureResult,
) -> Result<Value, Error> {
    let target = transactions
        .get(target_index as usize)
        .ok_or_else(|| invalid("target index out of range"))?;
    let selected: BTreeSet<_> = closure.selected_indices.iter().copied().collect();
    let mut dependencies: BTreeSet<_> = target
        .semantic_dependency_accounts
        .iter()
        .chain(std::iter::once(&target.fee_payer))
        .cloned()
        .collect();
    let mut effects = Vec::new();
    for index in (0..target_index as usize).rev() {
        let transaction = &transactions[index];
        if transaction.index != index as u64 {
            return Err(invalid("transaction index differs from position"));
        }
        if selected.contains(&(index as u64)) {
            dependencies.extend(
                transaction
                    .semantic_dependency_accounts
                    .iter()
                    .chain(&transaction.program_data_accounts)
                    .chain(std::iter::once(&transaction.fee_payer))
                    .cloned(),
            );
            continue;
        }
        if !transaction.succeeded {
            if let Some(writes) = &transaction.possible_persistent_writes {
                if let Some(key) = writes.iter().find(|key| dependencies.contains(*key)) {
                    return Err(Error::new(
                        "UNSUPPORTED_FAILED_DURABLE_NONCE_STATE",
                        format!("omitted persistent nonce write {index}:{key}"),
                    ));
                }
            }
        }
        if !dependencies.contains(&transaction.fee_payer) {
            continue;
        }
        if transaction.succeeded
            && transaction
                .application_writable_accounts
                .contains(&transaction.fee_payer)
        {
            return Err(invalid(format!(
                "live payer omitted from closure at {index}"
            )));
        }
        let archived = raw
            .get(index)
            .ok_or_else(|| invalid("archived transaction missing"))?;
        let meta = archived
            .get("meta")
            .ok_or_else(|| invalid("metadata missing"))?;
        if meta.get("err").is_none() || meta["err"].is_null() != transaction.succeeded {
            return Err(invalid("fee source status differs"));
        }
        let fee = amount(&meta["fee"])?;
        if fee == 0 {
            return Err(invalid("fee witness is zero"));
        }
        let pre = amounts(&meta["preBalances"])?;
        let post = amounts(&meta["postBalances"])?;
        if pre.is_empty()
            || pre.len() != transaction.declared_accounts.len()
            || post.len() != pre.len()
        {
            return Err(invalid("balance/account count differs"));
        }
        if pre[0].checked_sub(post[0]) != Some(fee) {
            return Err(invalid("fee-payer delta differs from fee"));
        }
        if !transaction.succeeded && pre[1..] != post[1..] {
            return Err(invalid("failed transaction changed non-fee balance"));
        }
        if archived["transaction"]["signatures"][0] != transaction.signature {
            return Err(invalid("fee signature differs"));
        }
        effects.push(json!({"index":index,"signature":transaction.signature,"feePayer":transaction.fee_payer,
            "feeLamports":fee.to_string(),"preLamports":pre[0].to_string(),"postLamports":post[0].to_string()}));
    }
    effects.reverse();
    Ok(
        json!({"omittedFeeEffectsHash":omitted_fee_effects_hash(&effects)?,"omittedFeeEffects":effects}),
    )
}

pub fn eligible_end_accounts(
    transactions: &[SemanticTransaction],
    indices: &[u64],
) -> Result<Vec<String>, Error> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for index in indices {
        let current = transactions
            .get(*index as usize)
            .ok_or_else(|| invalid("end evidence index out of range"))?;
        let mut later = BTreeSet::new();
        for tx in &transactions[*index as usize + 1..] {
            if tx.succeeded {
                later.extend(tx.writable_accounts.iter());
            } else {
                later.insert(&tx.fee_payer);
                if let Some(writes) = &tx.possible_persistent_writes {
                    later.extend(writes);
                }
            }
        }
        for key in &current.writable_accounts {
            if !later.contains(key) && seen.insert(key.clone()) {
                result.push(key.clone());
            }
        }
    }
    Ok(result)
}
