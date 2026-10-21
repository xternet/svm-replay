use super::*;

pub(in super::super) fn config(
    value: Option<&Value>,
    slot: u64,
    account_data: bool,
) -> Result<Value, RpcError> {
    let config = match value {
        None | Some(Value::Null) => json!({}),
        Some(value) => value.clone(),
    };
    let object = config
        .as_object()
        .ok_or_else(|| RpcError::params("config must be an object"))?;
    if object.keys().any(|key| {
        !["encoding", "dataSlice", "minContextSlot", "commitment"].contains(&key.as_str())
    }) {
        return Err(RpcError::params("unsupported RPC config field"));
    }
    if !account_data
        && object
            .keys()
            .any(|key| ["encoding", "dataSlice"].contains(&key.as_str()))
    {
        return Err(RpcError::params("this method has no account data options"));
    }
    // Rust RPC configs serialize unset Option fields as null.
    if let Some(commitment) = object.get("commitment").filter(|value| !value.is_null()) {
        if !["processed", "confirmed", "finalized"]
            .iter()
            .any(|valid| commitment == valid)
        {
            return Err(RpcError::params("invalid commitment"));
        }
    }
    if let Some(minimum) = object
        .get("minContextSlot")
        .filter(|value| !value.is_null())
    {
        let minimum = minimum
            .as_u64()
            .ok_or_else(|| RpcError::params("invalid minContextSlot"))?;
        if minimum > slot {
            return Err(RpcError::new(
                -32016,
                "Minimum context slot has not been reached",
            ));
        }
    }
    Ok(config)
}

fn u64_field(image: &Value, field: &str) -> Result<u64, RpcError> {
    image[field]
        .as_str()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| {
            RpcError::from(Error::new(
                "SESSION_ACCOUNT",
                format!("native account {field} missing or invalid"),
            ))
        })
}

/// A valid zstd frame (RFC 8878) holding the bytes in raw blocks: what Rust RPC
/// clients request by default, without a compression library.
pub(in super::super) fn zstd_frame(data: &[u8]) -> Vec<u8> {
    // Magic, single segment with an 8-byte content size, no checksum or dictionary.
    let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0xe0];
    frame.extend_from_slice(&(data.len() as u64).to_le_bytes());
    let mut chunks = data.chunks(128 * 1024).peekable();
    if chunks.peek().is_none() {
        frame.extend_from_slice(&[1, 0, 0]);
    }
    while let Some(chunk) = chunks.next() {
        // Block header: last-block bit, type 0 (raw), 21-bit size.
        let header = (chunk.len() as u32) << 3 | u32::from(chunks.peek().is_none());
        frame.extend_from_slice(&header.to_le_bytes()[..3]);
        frame.extend_from_slice(chunk);
    }
    frame
}

pub(in super::super) fn image(image: &Value, config: &Value) -> Result<Value, RpcError> {
    let invalid = || {
        RpcError::from(Error::new(
            "SESSION_ACCOUNT",
            "invalid native account image",
        ))
    };
    if image["presence"] != "present"
        || !image["owner"].is_string()
        || !image["executable"].is_boolean()
    {
        return Err(invalid());
    }
    let raw = image["dataBase64"].as_str().ok_or_else(invalid)?;
    let bytes = STANDARD.decode(raw).map_err(|_| invalid())?;
    if STANDARD.encode(&bytes) != raw {
        return Err(invalid());
    }
    let mut data = bytes.as_slice();
    if let Some(slice) = config.get("dataSlice").filter(|value| !value.is_null()) {
        let index = |field| {
            slice[field]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| {
                    RpcError::params("dataSlice requires non-negative offset and length")
                })
        };
        let offset = index("offset")?.min(data.len());
        let length = index("length")?.min(data.len() - offset);
        data = &data[offset..offset + length];
    }
    let encoding = match config.get("encoding").filter(|value| !value.is_null()) {
        None => "base58",
        Some(value) => value
            .as_str()
            .ok_or_else(|| RpcError::params("invalid account encoding"))?,
    };
    let encoded = match encoding {
        "base64" => STANDARD.encode(data),
        "base64+zstd" => STANDARD.encode(zstd_frame(data)),
        "base58" if data.len() <= 128 => bs58::encode(data).into_string(),
        "base58" => {
            return Err(RpcError::params(
                "base58 data exceeds 128 bytes; request base64",
            ))
        }
        _ => {
            return Err(RpcError::params(
                "supported account encodings: base64, base64+zstd, base58",
            ))
        }
    };
    Ok(
        json!({"data":[encoded,encoding],"executable":image["executable"],"owner":image["owner"],
        "lamports":u64_field(image,"lamports")?,"rentEpoch":u64_field(image,"rentEpoch")?,"space":bytes.len()}),
    )
}

pub(in super::super) fn accounts(
    session: &mut VerifiedSession,
    keys: &[Value],
    config: &Value,
    budget: &ExecutionBudget,
) -> Result<Vec<Value>, RpcError> {
    if keys.len() > 100 {
        return Err(RpcError::params("at most 100 account addresses"));
    }
    let keys = keys
        .iter()
        .map(|key| {
            let key = key
                .as_str()
                .ok_or_else(|| RpcError::params("account must be a base58 address"))?;
            crate::shared::history::address(key, 32)
                .map_err(|_| RpcError::params("invalid account address"))?;
            Ok(key.to_owned())
        })
        .collect::<Result<Vec<_>, RpcError>>()?;
    let values = session.request(
        SessionCommand::Accounts {
            pubkeys: keys.clone(),
        },
        budget,
    )?;
    let values = values
        .as_array()
        .filter(|values| values.len() == keys.len())
        .ok_or_else(|| {
            RpcError::from(Error::new(
                "SESSION_ACCOUNT",
                "native account response count differs",
            ))
        })?;
    values
        .iter()
        .map(|value| {
            if value.is_null() {
                Ok(Value::Null)
            } else {
                image(value, config)
            }
        })
        .collect()
}
