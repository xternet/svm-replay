use super::*;

pub(in super::super) type Check<T> = Result<T, String>;

pub(in super::super) const EPOCH_FIELDS: &[&str] = &[
    "schema",
    "genesisHash",
    "slot",
    "parentSlot",
    "blockhash",
    "blockEvidenceSha256",
    "executorSourceId",
    "runtimeProfileId",
    "activeFeatureSetHash",
    "clockDataSha256",
    "complete",
    "epoch",
    "totalStake",
    "voteStakes",
];

pub(in super::super) const INITIALIZED_FIELDS: &[&str] = &[
    "schema",
    "genesisHash",
    "slot",
    "parentSlot",
    "blockhash",
    "blockEvidenceSha256",
    "executorSourceId",
    "runtimeProfileId",
    "activeFeatureSetHash",
    "phase",
    "accounts",
];

pub(in super::super) const STAKE: &str = "Stake11111111111111111111111111111111111111";

pub(in super::super) fn require(condition: bool, message: &str) -> Check<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

pub(in super::super) fn object(value: &Value) -> Check<&Map<String, Value>> {
    value.as_object().ok_or_else(|| "expected object".into())
}

pub(in super::super) fn field<'a>(value: &'a Value, name: &str) -> Check<&'a Value> {
    object(value)?
        .get(name)
        .ok_or_else(|| format!("missing {name}"))
}

pub(in super::super) fn array(value: &Value) -> Check<&Vec<Value>> {
    value.as_array().ok_or_else(|| "expected array".into())
}

pub(in super::super) fn string(value: &Value) -> Check<&str> {
    value.as_str().ok_or_else(|| "expected string".into())
}

pub(in super::super) fn shape(value: &Value, fields: &[&str]) -> Check<()> {
    let value = object(value)?;
    require(
        value.len() == fields.len() && fields.iter().all(|key| value.contains_key(*key)),
        "unexpected/missing fields",
    )
}

pub(in super::super) fn sha256(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

pub(in super::super) fn pubkey(value: &Value) -> Check<&str> {
    let value = string(value)?;
    require(value.len() <= 44, "public key too long")?;
    let bytes = bs58::decode(value)
        .into_vec()
        .map_err(|error| format!("public key: {error}"))?;
    require(
        bytes.len() == 32 && bs58::encode(bytes).into_string() == value,
        "noncanonical public key",
    )?;
    Ok(value)
}

pub(in super::super) fn digest(value: &Value) -> Check<&str> {
    let value = string(value)?;
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "malformed hash",
    )?;
    Ok(value)
}

pub(in super::super) fn identifier(value: &Value, maximum: Option<usize>) -> Check<&str> {
    let value = string(value)?;
    require(
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte)),
        "invalid public identifier",
    )?;
    if let Some(maximum) = maximum {
        require(value.len() <= maximum, "public identifier too long")?;
    }
    Ok(value)
}

pub(in super::super) fn u64_string(value: &Value) -> Check<u64> {
    string(value)?;
    exact_u64(value)
}

pub(in super::super) fn slot(value: &Value) -> Check<u64> {
    let value = value
        .as_u64()
        .ok_or_else(|| "slot must be exact nonnegative number".to_owned())?;
    require(
        value <= 9_007_199_254_740_991,
        "slot exceeds safe integer range",
    )?;
    Ok(value)
}

pub(in super::super) fn bytes(value: &Value) -> Check<Vec<u8>> {
    let value = string(value)?;
    let bytes = STANDARD
        .decode(value)
        .map_err(|error| format!("malformed base64: {error}"))?;
    require(STANDARD.encode(&bytes) == value, "noncanonical base64")?;
    Ok(bytes)
}

pub(in super::super) fn equal(actual: &Value, expected: &Value, name: &str) -> Check<()> {
    require(actual == expected, &format!("{name} mismatch"))
}
