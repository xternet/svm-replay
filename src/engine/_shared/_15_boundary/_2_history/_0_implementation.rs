use super::*;
pub(in super::super) fn parents(job: &Job) -> Result<Images, Error> {
    array(field(job.fixture()?, "accounts")?)?
        .iter()
        .map(|a| {
            let value = match string(field(a, "presence")?)? {
                "absent" => None,
                "present" => Some(image(a)?),
                _ => return Err(invalid("unknown account presence")),
            };
            Ok((string(field(a, "pubkey")?)?.to_owned(), value))
        })
        .collect()
}
pub(in super::super) fn snapshot(job: &Job, first_use: bool) -> Result<Images, Error> {
    let mut images = Images::new();
    let mut found = false;
    for raw in array(&job.journal)? {
        let Some(event) = raw.get("event") else {
            continue;
        };
        let entries = if first_use {
            if event["kind"] != "enter" || event["parent_id"] != Value::Null {
                continue;
            }
            field(field(event, "invocation")?, "accounts")?
        } else {
            if event["kind"] != "transaction_begin" {
                continue;
            }
            found = true;
            field(event, "accounts")?
        };
        for a in array(entries)? {
            let k = string(field(a, "pubkey")?)?.to_owned();
            if !images.contains_key(&k) {
                images.insert(k, _0_images::journal_image(a)?);
            }
        }
        if !first_use {
            break;
        }
    }
    if !first_use && !found {
        return Err(invalid("transaction_begin missing"));
    }
    Ok(images)
}
pub(in super::super) fn touched(job: &Job) -> Result<BTreeSet<String>, Error> {
    let mut keys = BTreeSet::new();
    for tx in array(field(&job.result, "prefix")?)? {
        for t in array(field(tx, "accountTransitions")?)? {
            if field(t, "before")? != field(t, "after")? {
                keys.insert(string(field(t, "pubkey")?)?.to_owned());
            }
        }
    }
    if let Some(effects) = job.target()?.get("omittedFeeEffects") {
        for e in array(effects)? {
            keys.insert(string(field(e, "feePayer")?)?.to_owned());
        }
    }
    Ok(keys)
}
pub(in super::super) fn loaded(
    transaction: &str,
    parents: &Images,
) -> Result<(Vec<String>, Vec<String>), Error> {
    let decoded = svm_replay_protocol::transaction::decode(transaction)?;
    let message = &decoded["transaction"]["message"];
    let static_keys = array(field(message, "accountKeys")?)?
        .iter()
        .map(|v| Ok(string(v)?.to_owned()))
        .collect::<Result<_, Error>>()?;
    let mut extras = Vec::new();
    for lookup in array(field(message, "addressTableLookups")?)? {
        let address = string(field(lookup, "accountKey")?)?;
        let state = parents
            .get(address)
            .and_then(Option::as_ref)
            .ok_or_else(|| invalid(format!("lookup table {address} missing")))?;
        let data = bytes(field(state, "dataBase64")?)?;
        extras.push(address.to_owned());
        for index in array(field(lookup, "writableIndexes")?)?
            .iter()
            .chain(array(field(lookup, "readonlyIndexes")?)?)
        {
            let start =
                56 + usize::try_from(number(index)?).map_err(|e| invalid(e.to_string()))? * 32;
            let value = data
                .get(start..start + 32)
                .ok_or_else(|| invalid(format!("lookup table {address} entry missing")))?;
            extras.push(bs58::encode(value).into_string());
        }
    }
    Ok((static_keys, extras))
}
