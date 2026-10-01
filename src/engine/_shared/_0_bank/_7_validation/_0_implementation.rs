use super::*;

pub(in super::super) type Result<T> = std::result::Result<T, Error>;

pub(in super::super) fn fail(message: impl Into<String>) -> Error {
    Error::new("INVALID_BANK_INPUT", message)
}

pub(in super::super) fn check(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(fail(message))
    }
}

pub(in super::super) fn object(value: &Value) -> Result<&Map<String, Value>> {
    value.as_object().ok_or_else(|| fail("expected object"))
}

pub(in super::super) fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    object(value)?
        .get(name)
        .ok_or_else(|| fail(format!("missing {name}")))
}

pub(in super::super) fn array(value: &Value) -> Result<&Vec<Value>> {
    value.as_array().ok_or_else(|| fail("expected array"))
}

pub(in super::super) fn string(value: &Value) -> Result<&str> {
    value.as_str().ok_or_else(|| fail("expected string"))
}

pub(in super::super) fn uint(value: &Value) -> Result<u64> {
    exact_u64(value).map_err(fail)
}

pub(in super::super) fn integer(value: &Value) -> Result<u64> {
    let number = value
        .as_u64()
        .ok_or_else(|| fail("expected unsigned JSON integer"))?;
    check(
        number <= 9_007_199_254_740_991,
        "integer exceeds safe range",
    )?;
    Ok(number)
}

pub(in super::super) fn bytes(value: &Value) -> Result<Vec<u8>> {
    let text = string(value)?;
    let bytes = STANDARD
        .decode(text)
        .map_err(|e| fail(format!("base64: {e}")))?;
    check(STANDARD.encode(&bytes) == text, "noncanonical base64")?;
    Ok(bytes)
}

pub(in super::super) fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

pub(in super::super) fn digest(value: &str) -> Result<()> {
    check(
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "malformed hash",
    )
}

pub(in super::super) fn pubkey(value: &Value) -> Result<&str> {
    let text = string(value)?;
    let bytes = bs58::decode(text)
        .into_vec()
        .map_err(|e| fail(format!("base58: {e}")))?;
    check(
        bytes.len() == 32 && bs58::encode(bytes).into_string() == text,
        "malformed public key/hash",
    )?;
    Ok(text)
}
