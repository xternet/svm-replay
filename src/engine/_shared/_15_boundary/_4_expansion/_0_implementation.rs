use super::*;
pub(in super::super) fn companions(boundary: &Value) -> Result<Vec<String>, Error> {
    let mut wanted = BTreeSet::new();
    for a in object(field(boundary, "accounts")?)?.values() {
        let data = bytes(field(a, "dataBase64")?)?;
        let owner = string(field(a, "owner")?)?;
        let candidate = if owner == UPGRADEABLE
            && a["executable"] == true
            && data.len() >= 36
            && data[..4] == [2, 0, 0, 0]
        {
            Some(bs58::encode(&data[4..36]).into_string())
        } else if [
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
        ]
        .contains(&owner)
            && (data.len() == 165 || (data.len() > 165 && data[165] == 2))
        {
            Some(bs58::encode(&data[..32]).into_string())
        } else {
            None
        };
        if let Some(k) = candidate {
            if !known(boundary, &k)? {
                wanted.insert(k);
            }
        }
    }
    Ok(wanted.into_iter().collect())
}
pub(in super::super) fn known(b: &Value, k: &str) -> Result<bool, Error> {
    Ok(object(field(b, "accounts")?)?.contains_key(k)
        || array(field(b, "absent")?)?
            .iter()
            .chain(array(field(b, "unavailable")?)?)
            .any(|v| v == k))
}
fn compact(mut n: usize, out: &mut Vec<u8>) {
    loop {
        let byte = (n & 127) as u8;
        n >>= 7;
        out.push(byte | if n == 0 { 0 } else { 128 });
        if n == 0 {
            break;
        }
    }
}
pub(in super::super) fn footprint(transaction: &str, extra: &[String]) -> Result<String, Error> {
    if extra.is_empty() || extra.len() > 200 {
        return Err(Error::new(
            "INVALID_REQUEST",
            "expansion needs 1..200 accounts",
        ));
    }
    let decoded = svm_replay_protocol::transaction::decode(transaction)?;
    let message = &decoded["transaction"]["message"];
    let payer = string(&array(field(message, "accountKeys")?)?[0])?;
    let mut out = vec![1];
    out.extend([0; 64]);
    out.extend([1, 0, extra.len() as u8]);
    compact(1 + extra.len(), &mut out);
    out.extend(key(payer)?);
    for k in extra {
        out.extend(key(k)?);
    }
    out.extend(key(string(field(message, "recentBlockhash")?)?)?);
    out.push(0);
    Ok(STANDARD.encode(out))
}
pub(in super::super) fn widen(
    boundary: &mut Value,
    job: &Job,
    pending: &[String],
) -> Result<(), Error> {
    let parent = _2_history::parents(job)?;
    let touched = _2_history::touched(job)?;
    let begin = _2_history::snapshot(job, false)?;
    for k in pending {
        let state = if touched.contains(k) {
            begin.get(k)
        } else {
            parent.get(k)
        };
        match state {
            None if touched.contains(k) => array_mut(boundary, "unavailable")?.push(json!(k)),
            None => {
                return Err(Error::new(
                    "BOUNDARY_INCOMPLETE",
                    format!("SVM Replay did not prove {k} at this boundary"),
                ))
            }
            Some(None) => array_mut(boundary, "absent")?.push(json!(k)),
            Some(Some(a)) => {
                boundary["accounts"]
                    .as_object_mut()
                    .ok_or_else(|| invalid("accounts missing"))?
                    .insert(k.to_owned(), a.clone());
            }
        }
    }
    array_mut(boundary, "evidence")?.push(json!(job.path));
    Ok(())
}
fn array_mut<'a>(v: &'a mut Value, k: &str) -> Result<&'a mut Vec<Value>, Error> {
    v.get_mut(k)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid(format!("array {k} missing")))
}
