use super::*;

pub(in super::super::super) fn return_data(value: Option<&Value>, archived: bool) -> Result<Value> {
    let value = match value {
        Some(Value::Null) => return Ok(Value::Null),
        None if archived => return Ok(Value::Null), // RPC's documented omitted no-return field.
        None => return Err(VerificationError("missing return data".into())),
        Some(value) => value,
    };
    let program = string(field(value, "programId")?)?;
    let encoded = if archived {
        let data = array(field(value, "data")?)?;
        check(
            data.len() == 2 && data[1] == "base64",
            "return data encoding mismatch",
        )?;
        &data[0]
    } else {
        field(value, "dataBase64")?
    };
    let bytes = base64_bytes(encoded)?;
    if bytes.is_empty() {
        Ok(Value::Null)
    } else {
        Ok(json!({"programId":program, "dataBase64":encoded}))
    }
}

pub(in super::super::super) fn nullable_unsigned(value: &Value) -> Result<Option<u64>> {
    if value.is_null() {
        Ok(None)
    } else {
        unsigned(value).map(Some)
    }
}

pub(in super::super::super) fn state_lamports(value: &Value) -> Result<u64> {
    match string(field(value, "presence")?)? {
        "absent" => {
            for key in [
                "lamports",
                "owner",
                "executable",
                "rentEpoch",
                "dataHash",
                "tokenAmount",
            ] {
                check(
                    field(value, key)?.is_null(),
                    &format!("absent state contains {key}"),
                )?;
            }
            Ok(0)
        }
        "present" => {
            string(field(value, "owner")?)?;
            check(
                field(value, "executable")?.is_boolean(),
                "state executable malformed",
            )?;
            unsigned(field(value, "rentEpoch")?)?;
            let data_hash = string(field(value, "dataHash")?)?;
            check(
                data_hash.len() == 64
                    && data_hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
                "state data hash malformed",
            )?;
            nullable_unsigned(field(value, "tokenAmount")?)?;
            unsigned(field(value, "lamports")?)
        }
        _ => Err(VerificationError("unknown state presence".into())),
    }
}

pub(in super::super::super) fn token_amounts(
    value: &Value,
    account_count: usize,
) -> Result<BTreeMap<usize, u64>> {
    let mut amounts = BTreeMap::new();
    for entry in array(value)? {
        let index = usize_value(field(entry, "accountIndex")?)?;
        check(index < account_count, "token index out of range")?;
        let amount = unsigned(field(field(entry, "uiTokenAmount")?, "amount")?)?;
        check(
            amounts.insert(index, amount).is_none(),
            "duplicate token index",
        )?;
    }
    Ok(amounts)
}

pub(in super::super::super) fn archived_state(expected: &Value, actual: &Value) -> Result<()> {
    state_lamports(actual)?;
    equal(
        field(actual, "presence")?,
        field(expected, "presence")?,
        "end presence",
    )?;
    if field(expected, "presence")? == "absent" {
        return Ok(());
    }
    check(
        unsigned(field(actual, "lamports")?)? == unsigned(field(expected, "lamports")?)?,
        "end lamports mismatch",
    )?;
    equal(
        field(actual, "owner")?,
        field(expected, "owner")?,
        "end owner",
    )?;
    equal(
        field(actual, "executable")?,
        field(expected, "executable")?,
        "end executable",
    )?;
    check(
        unsigned(field(actual, "rentEpoch")?)? == unsigned(field(expected, "rentEpoch")?)?,
        "end rent epoch mismatch",
    )?;
    equal(
        field(actual, "dataHash")?,
        &json!(hash(base64_bytes(field(expected, "dataBase64")?)?)),
        "end data hash",
    )
}
