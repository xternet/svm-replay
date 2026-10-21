use super::*;
pub(in super::super) type Images = BTreeMap<String, Option<Value>>;
pub(in super::super) fn field<'a>(v: &'a Value, name: &str) -> Result<&'a Value, Error> {
    v.get(name)
        .ok_or_else(|| invalid(format!("missing {name}")))
}
pub(in super::super) fn object(v: &Value) -> Result<&serde_json::Map<String, Value>, Error> {
    v.as_object().ok_or_else(|| invalid("expected object"))
}
pub(in super::super) fn array(v: &Value) -> Result<&Vec<Value>, Error> {
    v.as_array().ok_or_else(|| invalid("expected array"))
}
pub(in super::super) fn string(v: &Value) -> Result<&str, Error> {
    v.as_str().ok_or_else(|| invalid("expected string"))
}
pub(in super::super) fn number(v: &Value) -> Result<u64, Error> {
    v.as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| invalid("expected safe unsigned integer"))
}
pub(in super::super) fn key(s: &str) -> Result<Vec<u8>, Error> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| invalid(e.to_string()))?;
    if bytes.len() != 32 {
        return Err(invalid(format!("expected 32-byte address: {s}")));
    }
    Ok(bytes)
}
pub(in super::super) fn bytes(v: &Value) -> Result<Vec<u8>, Error> {
    let encoded = string(v)?;
    let data = STANDARD
        .decode(encoded)
        .map_err(|e| invalid(e.to_string()))?;
    if STANDARD.encode(&data) != encoded {
        return Err(invalid("noncanonical base64"));
    }
    Ok(data)
}
fn decimal(v: &Value) -> Result<String, Error> {
    let s = v
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| v.to_string());
    let n = s.parse::<u64>().map_err(|e| invalid(e.to_string()))?;
    if n.to_string() != s {
        return Err(invalid("noncanonical u64"));
    }
    Ok(s)
}
pub(in super::super) fn image(v: &Value) -> Result<Value, Error> {
    let pubkey = string(field(v, "pubkey")?)?;
    let owner = string(field(v, "owner")?)?;
    key(pubkey)?;
    key(owner)?;
    let executable = field(v, "executable")?
        .as_bool()
        .ok_or_else(|| invalid("missing executable"))?;
    Ok(
        json!({"pubkey":pubkey,"owner":owner,"executable":executable,
        "lamports":decimal(field(v,"lamports")?)?,"rentEpoch":decimal(field(v,"rentEpoch")?)?,
        "dataBase64":STANDARD.encode(bytes(field(v,"dataBase64")?)?)}),
    )
}
pub(in super::super) fn journal_image(v: &Value) -> Result<Option<Value>, Error> {
    let pubkey = string(field(v, "pubkey")?)?;
    let state = field(v, "state")?;
    if state.is_null() {
        return Ok(None);
    }
    let lamports = decimal(field(state, "lamports")?)?;
    if lamports == "0" {
        return Ok(None);
    }
    let data: Vec<u8> = array(field(state, "data")?)?
        .iter()
        .map(|v| {
            v.as_u64()
                .and_then(|v| u8::try_from(v).ok())
                .ok_or_else(|| invalid("invalid account byte"))
        })
        .collect::<Result<_, _>>()?;
    Ok(Some(image(
        &json!({"pubkey":pubkey,"owner":field(state,"owner")?,"executable":field(state,"executable")?,
        "lamports":lamports,"rentEpoch":decimal(field(state,"rent_epoch")?)?,"dataBase64":STANDARD.encode(data)}),
    )?))
}
