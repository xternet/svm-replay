use super::*;

const SYSTEM: &str = "11111111111111111111111111111111";
fn invalid(message: impl Into<String>) -> Error {
    Error::new("CREDIT_EVIDENCE", message)
}
fn array<'a>(value: &'a Value, name: &str) -> Result<&'a Vec<Value>, Error> {
    value
        .as_array()
        .ok_or_else(|| invalid(format!("missing {name}")))
}
fn amount(value: &Value) -> Result<u64, Error> {
    crate::shared::diff::exact_u64(value).map_err(invalid)
}

/// Prove credit-only changes to a present empty System account over a prefix.
/// The caller must additionally bind runtime semantics and chronological use.
/// `None` means ineligible; malformed supplied evidence is an explicit error.
pub fn prove(parent: &Value, raw_prefix: &[Value]) -> Result<Option<Value>, Error> {
    if parent["presence"] != "present"
        || parent["owner"] != SYSTEM
        || parent["dataBase64"] != ""
        || parent["executable"] != false
        || parent["rentEpoch"] != u64::MAX.to_string()
    {
        return Ok(None);
    }
    let key = parent["pubkey"]
        .as_str()
        .ok_or_else(|| invalid("parent pubkey"))?;
    let mut balance = amount(&parent["lamports"])?;
    if balance == 0 {
        return Ok(None);
    }
    let mut credits = Vec::new();
    for (index, raw) in raw_prefix.iter().enumerate() {
        let message = &raw["transaction"]["message"];
        let meta = &raw["meta"];
        let mut keys = array(&message["accountKeys"], "account keys")?.clone();
        if !meta["loadedAddresses"].is_null() {
            keys.extend(array(&meta["loadedAddresses"]["writable"], "loaded writable")?.clone());
            keys.extend(array(&meta["loadedAddresses"]["readonly"], "loaded readonly")?.clone());
        }
        let Some(position) = keys.iter().position(|k| k == key) else {
            continue;
        };
        if position == 0 {
            return Ok(None);
        }
        let pre = array(&meta["preBalances"], "pre balances")?;
        let post = array(&meta["postBalances"], "post balances")?;
        if pre.len() != keys.len() || post.len() != keys.len() {
            return Err(invalid("balance/account count differs"));
        }
        let before = amount(&pre[position])?;
        let after = amount(&post[position])?;
        if before != balance || after < before {
            return Ok(None);
        }
        let error = meta
            .get("err")
            .ok_or_else(|| invalid("transaction status missing"))?;
        if !error.is_null() {
            if after != before {
                return Ok(None);
            }
            continue;
        }
        let Some(inner) = meta["innerInstructions"].as_array() else {
            return Ok(None);
        };
        let mut instructions: Vec<&Value> = array(&message["instructions"], "instructions")?
            .iter()
            .collect();
        for group in inner {
            instructions.extend(array(&group["instructions"], "inner instructions")?);
        }
        for instruction in instructions {
            let program = instruction["programIdIndex"]
                .as_u64()
                .and_then(|i| keys.get(i as usize))
                .ok_or_else(|| invalid("program index"))?;
            let accounts = array(&instruction["accounts"], "instruction accounts")?;
            if program != SYSTEM || !accounts.contains(&json!(position)) {
                continue;
            }
            let data = bs58::decode(
                instruction["data"]
                    .as_str()
                    .ok_or_else(|| invalid("instruction data"))?,
            )
            .into_vec()
            .map_err(|e| invalid(format!("instruction encoding: {e}")))?;
            if data.len() != 12
                || data[..4] != [2, 0, 0, 0]
                || accounts.len() != 2
                || accounts[1] != position
                || accounts[0] == position
            {
                return Ok(None);
            }
        }
        if after != before {
            let signature = raw["transaction"]["signatures"][0]
                .as_str()
                .ok_or_else(|| invalid("signature missing"))?;
            credits.push(json!({"index":index,"signature":signature,
                "preLamports":before.to_string(),"postLamports":after.to_string(),
                "lamports":(after-before).to_string()}));
        }
        balance = after;
    }
    Ok(Some(
        json!({"schema":"svm-system-credit-proof/v1","pubkey":key,
        "parent":parent,"credits":credits,"finalLamports":balance.to_string()}),
    ))
}
