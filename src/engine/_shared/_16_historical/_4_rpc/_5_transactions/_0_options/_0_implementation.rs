use super::*;

pub(in super::super) fn flag(config: &Value, name: &str, default: bool) -> Result<bool, RpcError> {
    match config.get(name).filter(|v| !v.is_null()) {
        None => Ok(default),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| RpcError::params(format!("{name} must be boolean"))),
    }
}

pub(in super::super) fn config(
    value: Option<&Value>,
    slot: u64,
    simulate: bool,
) -> Result<Value, RpcError> {
    let config = match value {
        None | Some(Value::Null) => json!({}),
        Some(value) => value.clone(),
    };
    let object = config
        .as_object()
        .ok_or_else(|| RpcError::params("config must be an object"))?;
    let allowed = if simulate {
        &[
            "encoding",
            "sigVerify",
            "replaceRecentBlockhash",
            "commitment",
            "minContextSlot",
            "accounts",
            "innerInstructions",
        ][..]
    } else {
        &[
            "encoding",
            "skipPreflight",
            "preflightCommitment",
            "maxRetries",
            "minContextSlot",
        ][..]
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(RpcError::params("unsupported transaction option"));
    }
    let commitment = if simulate {
        "commitment"
    } else {
        "preflightCommitment"
    };
    let common =
        json!({"commitment":object.get(commitment),"minContextSlot":object.get("minContextSlot")});
    super::super::super::_2_accounts::config(Some(&common), slot, false)?;
    if simulate {
        if flag(&config, "sigVerify", false)? && flag(&config, "replaceRecentBlockhash", false)? {
            return Err(RpcError::params(
                "sigVerify and replaceRecentBlockhash are mutually exclusive",
            ));
        }
        if flag(&config, "innerInstructions", false)?
            || object.get("accounts").is_some_and(|v| !v.is_null())
        {
            return Err(RpcError::params("simulation account/inner-instruction output is not available in this native RPC version"));
        }
    } else {
        flag(&config, "skipPreflight", false)?;
        // The fork executes a transaction once, on arrival: nothing is rebroadcast.
        if let Some(retries) = object.get("maxRetries").filter(|v| !v.is_null()) {
            if retries.as_u64().is_none() {
                return Err(RpcError::params(
                    "maxRetries must be a non-negative integer",
                ));
            }
        }
    }
    Ok(config)
}

pub(in super::super) fn wire(value: &Value, config: &Value) -> Result<String, RpcError> {
    let value = value
        .as_str()
        .ok_or_else(|| RpcError::params("transaction must be an encoded string"))?;
    if value.len() > 1700 {
        return Err(RpcError::params("transaction exceeds packet limit"));
    }
    let encoding = match config.get("encoding").filter(|v| !v.is_null()) {
        None => "base58",
        Some(value) => value
            .as_str()
            .ok_or_else(|| RpcError::params("invalid encoding"))?,
    };
    let bytes = match encoding {
        "base64" => {
            let bytes = STANDARD
                .decode(value)
                .map_err(|_| RpcError::params("invalid base64 transaction"))?;
            if STANDARD.encode(&bytes) != value {
                return Err(RpcError::params("noncanonical base64 transaction"));
            }
            bytes
        }
        "base58" => {
            let bytes = bs58::decode(value)
                .into_vec()
                .map_err(|_| RpcError::params("invalid base58 transaction"))?;
            if bs58::encode(&bytes).into_string() != value {
                return Err(RpcError::params("noncanonical base58 transaction"));
            }
            bytes
        }
        _ => {
            return Err(RpcError::params(
                "supported transaction encodings: base64, base58",
            ))
        }
    };
    if bytes.is_empty() || bytes.len() > 1232 {
        return Err(RpcError::params("invalid transaction packet size"));
    }
    Ok(STANDARD.encode(bytes))
}
