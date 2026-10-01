use super::*;

/// Reviewed ownership enforcement, System transfer and rollback semantics.
pub fn supports_credit_preloads(executor: &str) -> bool {
    matches!(
        executor,
        "litesvm-v0.7.1-agave-2.3.9"
            | "litesvm-v0.8.2-agave-3.0.10"
            | "litesvm-v0.12.0-agave-3.1.11"
            | "litesvm-v0.13.1-agave-4.0.0"
            | "litesvm-v0.14.0-pr402-agave-4.1.2"
            | "litesvm-v0.16.0-agave-4.2.1"
    )
}

fn invalid(message: &str) -> Error {
    Error::new("CREDIT_EVIDENCE", message)
}
fn number(value: &Value) -> Result<u64, Error> {
    value
        .as_u64()
        .ok_or_else(|| invalid("preload index missing"))
}

pub fn validate_preloads(fixture: &Value, raw: &[Value]) -> Result<(), Error> {
    let Some(value) = fixture["runtime"].get("systemCreditPreloads") else {
        return Ok(());
    };
    if !fixture["runtime"]["binding"]["executor"]["id"]
        .as_str()
        .is_some_and(supports_credit_preloads)
    {
        return Err(invalid("unreviewed credit preload runtime"));
    }
    let proofs = value
        .as_array()
        .ok_or_else(|| invalid("preloads must be an array"))?;
    let target = number(&fixture["target"]["index"])?;
    let prefix = raw
        .get(..target as usize)
        .ok_or_else(|| invalid("preload target outside block"))?;
    let indices = fixture["target"]["prefixIndices"]
        .as_array()
        .ok_or_else(|| invalid("prefix indices"))?;
    let mut selected = indices
        .iter()
        .map(number)
        .collect::<Result<BTreeSet<_>, _>>()?;
    selected.insert(target);
    let accounts = fixture["accounts"]
        .as_array()
        .ok_or_else(|| invalid("fixture accounts"))?;
    let mut seen = BTreeSet::new();
    for supplied in proofs {
        let mut expected = prove(&supplied["parent"], prefix)?
            .ok_or_else(|| invalid("credit field invariance is not proven"))?;
        let key = expected["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("preload key"))?
            .to_owned();
        if !seen.insert(key.clone()) {
            return Err(invalid("duplicate preload"));
        }
        let mut first = target;
        for &i in &selected {
            let tx = raw
                .get(i as usize)
                .ok_or_else(|| invalid("preload use outside block"))?;
            if summarize_semantic_transaction(tx, i)?
                .declared_accounts
                .contains(&key)
            {
                first = i;
                break;
            }
        }
        let credits = expected["credits"]
            .as_array()
            .ok_or_else(|| invalid("credits missing"))?;
        let mut applied = Vec::new();
        let mut image = supplied["parent"].clone();
        let mut balance = crate::shared::diff::exact_u64(&image["lamports"])
            .map_err(|_| invalid("parent amount"))?;
        for credit in credits {
            let i = number(&credit["index"])?;
            if selected.contains(&i) {
                continue;
            }
            if i >= first {
                return Err(invalid("credit after first replay use"));
            }
            let amount = crate::shared::diff::exact_u64(&credit["lamports"])
                .map_err(|_| invalid("credit amount"))?;
            balance = balance
                .checked_add(amount)
                .ok_or_else(|| invalid("credit overflow"))?;
            applied.push(credit.clone());
        }
        expected["firstUse"] = json!(first);
        expected["appliedCredits"] = json!(applied);
        expected["initializedLamports"] = json!(balance.to_string());
        if &expected != supplied {
            return Err(invalid("preload proof differs from archived prefix"));
        }
        image["lamports"] = json!(balance.to_string());
        let actual = accounts
            .iter()
            .find(|a| a["pubkey"] == key)
            .ok_or_else(|| invalid("preload account missing"))?;
        for field in [
            "presence",
            "sourceSlot",
            "owner",
            "dataBase64",
            "executable",
            "rentEpoch",
            "lamports",
        ] {
            if actual[field] != image[field] {
                return Err(invalid("preload image differs from proof"));
            }
        }
    }
    Ok(())
}
