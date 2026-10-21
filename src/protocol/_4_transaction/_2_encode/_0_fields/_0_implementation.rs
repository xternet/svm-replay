use super::*;

pub(in super::super) fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    value
        .get(name)
        .ok_or_else(|| invalid("missing archived wire field"))
}
pub(in super::super) fn array(value: &Value) -> Result<&Vec<Value>, Error> {
    value
        .as_array()
        .ok_or_else(|| invalid("archived wire array required"))
}
pub(in super::super) fn uint(value: &Value, width: usize) -> Result<Vec<u8>, Error> {
    let value = value
        .as_u64()
        .ok_or_else(|| invalid("archived wire integer required"))?;
    if width < 8 && value >= 1_u64 << (width * 8) {
        return Err(invalid("archived wire integer overflow"));
    }
    Ok(value.to_le_bytes()[..width].to_vec())
}
pub(in super::super) fn byte(value: &Value) -> Result<u8, Error> {
    Ok(uint(value, 1)?[0])
}
pub(in super::super) fn shortvec(size: usize) -> Result<Vec<u8>, Error> {
    let mut size = u16::try_from(size).map_err(|_| invalid("archived shortvec overflow"))?;
    let mut bytes = Vec::new();
    loop {
        let mut part = (size & 127) as u8;
        size >>= 7;
        if size != 0 {
            part |= 128;
        }
        bytes.push(part);
        if size == 0 {
            return Ok(bytes);
        }
    }
}
pub(in super::super) fn data(value: &Value, width: Option<usize>) -> Result<Vec<u8>, Error> {
    let value = value
        .as_str()
        .ok_or_else(|| invalid("archived base58 string required"))?;
    let bytes = bs58::decode(value)
        .into_vec()
        .map_err(|_| invalid("invalid archived base58"))?;
    if width.is_some_and(|width| width != bytes.len())
        || bs58::encode(&bytes).into_string() != value
    {
        return Err(invalid("noncanonical or wrong-width archived base58"));
    }
    Ok(bytes)
}
pub(in super::super) fn header(message: &Value) -> Result<Vec<u8>, Error> {
    let header = field(message, "header")?;
    [
        "numRequiredSignatures",
        "numReadonlySignedAccounts",
        "numReadonlyUnsignedAccounts",
    ]
    .iter()
    .map(|name| byte(field(header, name)?))
    .collect()
}
pub(in super::super) fn keys(message: &Value) -> Result<Vec<Vec<u8>>, Error> {
    array(field(message, "accountKeys")?)?
        .iter()
        .map(|key| {
            let key = if key.is_object() {
                field(key, "pubkey")?
            } else {
                key
            };
            data(key, Some(32))
        })
        .collect()
}
pub(in super::super) fn signatures(transaction: &Value) -> Result<Vec<Vec<u8>>, Error> {
    array(field(transaction, "signatures")?)?
        .iter()
        .map(|key| data(key, Some(64)))
        .collect()
}
