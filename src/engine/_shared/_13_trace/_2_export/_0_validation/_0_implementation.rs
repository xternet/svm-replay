use super::*;

pub struct CaptureExport {
    pub artifact: Value,
    pub payload: Vec<u8>,
}

pub(in super::super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("CAPTURE_EXPORT", message)
}

pub(in super::super) fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    value
        .get(name)
        .ok_or_else(|| invalid(format!("missing {name}")))
}

pub(in super::super) fn array(value: &Value) -> Result<&[Value], Error> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("expected array"))
}

pub(in super::super) fn string(value: &Value) -> Result<&str, Error> {
    value.as_str().ok_or_else(|| invalid("expected string"))
}

pub(in super::super) fn unsigned(value: &Value, numeric_only: bool) -> Result<u64, Error> {
    let text = match value {
        Value::String(text) if !numeric_only => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => return Err(invalid("invalid unsigned runtime token")),
    };
    check(
        !text.is_empty()
            && text.bytes().all(|byte| byte.is_ascii_digit())
            && (text == "0" || !text.starts_with('0')),
        "unsigned runtime token is not canonical",
    )?;
    text.parse()
        .map_err(|error| invalid(format!("unsigned runtime overflow: {error}")))
}

pub(in super::super) fn raw_bytes(value: &Value) -> Result<(), Error> {
    for byte in array(value)? {
        check(
            byte.as_u64().is_some_and(|byte| byte <= 255),
            "raw byte missing/invalid",
        )?;
    }
    Ok(())
}

pub(in super::super) fn accounts(value: &Value, expected: Option<&Value>) -> Result<(), Error> {
    let mut keys = BTreeSet::new();
    for account in array(value)? {
        let id = string(field(account, "pubkey")?)?;
        key(id)?;
        check(keys.insert(id), "duplicate raw account snapshot")?;
        let state = field(account, "state")?;
        if state.is_null() {
            continue;
        }
        unsigned(field(state, "lamports")?, false)?;
        unsigned(field(state, "rent_epoch")?, false)?;
        key(string(field(state, "owner")?)?)?;
        check(
            field(state, "executable")?.is_boolean(),
            "account executable flag missing",
        )?;
        raw_bytes(field(state, "data")?)?;
    }
    if let Some(expected) = expected {
        let expected = array(expected)?
            .iter()
            .map(string)
            .collect::<Result<Vec<_>, _>>()?;
        check(
            expected.iter().copied().collect::<BTreeSet<_>>() == keys
                && expected.len() == keys.len(),
            "raw boundary keys differ from declared scope",
        )?;
    }
    Ok(())
}

pub(in super::super) fn outcome(value: &Value) -> Result<(), Error> {
    check(
        value["status"] == "success" || value["status"] == "failure" && value["error"].is_string(),
        "runtime outcome missing",
    )
}

pub(in super::super) fn parent(value: &Value) -> Result<Option<u64>, Error> {
    if value.is_null() {
        Ok(None)
    } else {
        unsigned(value, false).map(Some)
    }
}
