use super::*;

pub struct CreditPlan {
    pub closure: SemanticClosureResult,
    pub images: Vec<Value>,
    pub proofs: Vec<Value>,
}
fn invalid(message: &str) -> Error {
    Error::new("CREDIT_EVIDENCE", message)
}
fn index(value: &Value) -> Result<u64, Error> {
    value
        .as_u64()
        .ok_or_else(|| invalid("credit index missing"))
}
fn credits(proof: &Value) -> Result<&Vec<Value>, Error> {
    proof["credits"]
        .as_array()
        .ok_or_else(|| invalid("credit list missing"))
}

/// A preload is safe only if no omitted balance changes occur after first use.
pub fn plan(
    parents: &[Value],
    raw: &[Value],
    transactions: &[SemanticTransaction],
    target: u64,
    initial: &SemanticClosureResult,
) -> Result<CreditPlan, Error> {
    if raw.len() != transactions.len()
        || target >= transactions.len() as u64
        || transactions
            .iter()
            .enumerate()
            .any(|(i, tx)| tx.index != i as u64)
    {
        return Err(invalid(
            "credit planner requires contiguous block transaction indices",
        ));
    }
    let prefix = raw
        .get(..target as usize)
        .ok_or_else(|| invalid("target outside block"))?;
    let mut candidates = BTreeMap::new();
    for parent in parents {
        if let Some(proof) = prove(parent, prefix)? {
            if credits(&proof)?.is_empty() {
                continue;
            }
            let key = proof["pubkey"]
                .as_str()
                .ok_or_else(|| invalid("proof key missing"))?;
            candidates.insert(key.to_owned(), proof);
        }
    }
    loop {
        if candidates.is_empty() {
            return Ok(CreditPlan {
                closure: initial.clone(),
                images: vec![],
                proofs: vec![],
            });
        }
        let mut projected = transactions.to_vec();
        for tx in &mut projected {
            tx.application_writable_accounts
                .retain(|key| !candidates.contains_key(key));
        }
        let closure =
            compute_backward_semantic_closure(&projected, target, &initial.program_data_accounts)?;
        if closure
            .selected_indices
            .iter()
            .any(|i| !initial.selected_indices.contains(i))
        {
            return Err(invalid(
                "credit closure introduced an unexpected predecessor",
            ));
        }
        let selected: BTreeSet<_> = closure
            .selected_indices
            .iter()
            .copied()
            .chain([target])
            .collect();
        let mut rejected = Vec::new();
        let mut images = Vec::new();
        let mut proofs = Vec::new();
        for (key, proof) in &candidates {
            let first = selected
                .iter()
                .copied()
                .find(|i| transactions[*i as usize].declared_accounts.contains(key));
            let Some(first) = first else {
                continue;
            };
            let omitted = credits(proof)?
                .iter()
                .filter_map(|credit| match index(&credit["index"]) {
                    Ok(i) if selected.contains(&i) => None,
                    Ok(_) => Some(Ok(credit.clone())),
                    Err(e) => Some(Err(e)),
                })
                .collect::<Result<Vec<_>, Error>>()?;
            if omitted
                .iter()
                .any(|c| c["index"].as_u64().is_some_and(|i| i >= first))
            {
                rejected.push(key.clone());
                continue;
            }
            if omitted.is_empty() {
                continue;
            }
            let mut image = proof["parent"].clone();
            let mut balance = crate::shared::diff::exact_u64(&image["lamports"])
                .map_err(|_| invalid("parent amount"))?;
            for credit in &omitted {
                let amount = crate::shared::diff::exact_u64(&credit["lamports"])
                    .map_err(|_| invalid("credit amount"))?;
                balance = balance
                    .checked_add(amount)
                    .ok_or_else(|| invalid("credit overflow"))?;
            }
            image["lamports"] = json!(balance.to_string());
            let mut bound = proof.clone();
            bound["firstUse"] = json!(first);
            bound["appliedCredits"] = json!(omitted);
            bound["initializedLamports"] = json!(balance.to_string());
            proofs.push(bound);
            images.push(image);
        }
        if rejected.is_empty() {
            return Ok(CreditPlan {
                closure,
                images,
                proofs,
            });
        }
        for key in rejected {
            candidates.remove(&key);
        }
    }
}
