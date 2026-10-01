use super::*;

pub(super) const RENT: &str = "SysvarRent111111111111111111111111111111111";

pub(super) const RESTART: &str = "SysvarLastRestartS1ot1111111111111111111111";

pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("SOURCE_PREPARATION", message)
}

pub(super) fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>, Error> {
    value
        .as_array()
        .ok_or_else(|| invalid(format!("{label} missing/malformed")))
}

pub(super) fn strings(value: &Value) -> Result<Vec<String>, Error> {
    array(value, "strings")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("string missing"))
        })
        .collect()
}

pub(super) fn unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|v| seen.insert(v.clone()))
        .collect()
}

pub(super) fn required<'a>(values: &'a [Value], key: &str) -> Result<&'a Value, Error> {
    let mut matching = values.iter().filter(|v| v["pubkey"] == key);
    let value = matching
        .next()
        .ok_or_else(|| invalid(format!("required account missing:{key}")))?;
    if matching.next().is_some() {
        return Err(invalid("duplicate account"));
    }
    Ok(value)
}

pub struct Prepared {
    pub request: PreparedRequest,
    pub receipt: Value,
}
