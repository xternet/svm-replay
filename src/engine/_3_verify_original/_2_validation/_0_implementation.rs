use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationError(pub String);

impl std::fmt::Display for VerificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for VerificationError {}

pub(in super::super) type Result<T> = std::result::Result<T, VerificationError>;

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct VerificationReport(pub Value);

pub(in super::super) fn check(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(VerificationError(message.into()))
    }
}

pub(in super::super) fn object(value: &Value) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| VerificationError("expected object".into()))
}

pub(in super::super) fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    object(value)?
        .get(name)
        .ok_or_else(|| VerificationError(format!("missing {name}")))
}

pub(in super::super) fn array(value: &Value) -> Result<&Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| VerificationError("expected array".into()))
}

pub(in super::super) fn string(value: &Value) -> Result<&str> {
    value
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| VerificationError("expected nonempty string".into()))
}

pub(in super::super) fn strings(value: &Value) -> Result<Vec<String>> {
    array(value)?
        .iter()
        .map(|value| string(value).map(str::to_owned))
        .collect()
}

pub(in super::super) fn unsigned(value: &Value) -> Result<u64> {
    exact_u64(value).map_err(VerificationError)
}

pub(in super::super) fn integer(value: &Value) -> Result<u64> {
    let value = value
        .as_u64()
        .ok_or_else(|| VerificationError("expected numeric unsigned integer".into()))?;
    check(
        value <= 9_007_199_254_740_991,
        "integer exceeds safe index/CU range",
    )?;
    Ok(value)
}

pub(in super::super) fn usize_value(value: &Value) -> Result<usize> {
    usize::try_from(integer(value)?)
        .map_err(|error| VerificationError(format!("index out of range: {error}")))
}

pub(in super::super) fn equal(actual: &Value, expected: &Value, label: &str) -> Result<()> {
    check(
        actual == expected,
        &format!("{label} mismatch: expected {expected}, observed {actual}"),
    )
}

pub(in super::super) fn base64_bytes(value: &Value) -> Result<Vec<u8>> {
    let value = value
        .as_str()
        .ok_or_else(|| VerificationError("base64 must be a string".into()))?;
    let bytes = STANDARD
        .decode(value)
        .map_err(|error| VerificationError(format!("malformed base64: {error}")))?;
    check(STANDARD.encode(&bytes) == value, "noncanonical base64")?;
    Ok(bytes)
}

pub(in super::super) fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
