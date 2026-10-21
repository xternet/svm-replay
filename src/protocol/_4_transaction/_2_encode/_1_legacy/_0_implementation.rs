use super::*;

pub(in super::super) fn encode(archive: &Value, versioned: bool) -> Result<Vec<u8>, Error> {
    let transaction = field(archive, "transaction")?;
    let message = field(transaction, "message")?;
    let signatures = signatures(transaction)?;
    let keys = keys(message)?;
    let mut bytes = shortvec(signatures.len())?;
    for signature in signatures {
        bytes.extend(signature);
    }
    if versioned {
        bytes.push(128);
    }
    bytes.extend(header(message)?);
    bytes.extend(shortvec(keys.len())?);
    for key in keys {
        bytes.extend(key);
    }
    bytes.extend(data(field(message, "recentBlockhash")?, Some(32))?);
    let instructions = array(field(message, "instructions")?)?;
    bytes.extend(shortvec(instructions.len())?);
    for ix in instructions {
        bytes.push(byte(field(ix, "programIdIndex")?)?);
        let accounts = array(field(ix, "accounts")?)?;
        bytes.extend(shortvec(accounts.len())?);
        for account in accounts {
            bytes.push(byte(account)?);
        }
        let data = data(field(ix, "data")?, None)?;
        bytes.extend(shortvec(data.len())?);
        bytes.extend(data);
    }
    if versioned {
        // Empty lookups are an optional RPC representation of an empty wire list.
        let lookups = match message.get("addressTableLookups") {
            Some(value) => array(value)?.as_slice(),
            None => &[],
        };
        bytes.extend(shortvec(lookups.len())?);
        for lookup in lookups {
            bytes.extend(data(field(lookup, "accountKey")?, Some(32))?);
            for kind in ["writableIndexes", "readonlyIndexes"] {
                let indexes = array(field(lookup, kind)?)?;
                bytes.extend(shortvec(indexes.len())?);
                for index in indexes {
                    bytes.push(byte(index)?);
                }
            }
        }
    }
    Ok(bytes)
}
