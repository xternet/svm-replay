use super::*;

pub(in super::super) const LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

pub const OVERRIDABLE_SYSVARS: &[&str] = &[
    "SysvarC1ock11111111111111111111111111111111",
    "SysvarEpochSchedu1e111111111111111111111111",
    "SysvarEpochRewards1111111111111111111111111",
    "SysvarRent111111111111111111111111111111111",
    "SysvarS1otHashes111111111111111111111111111",
    "SysvarStakeHistory1111111111111111111111111",
    "SysvarLastRestartS1ot1111111111111111111111",
    "SysvarS1otHistory11111111111111111111111111",
    "SysvarFees111111111111111111111111111111111",
    "SysvarRecentB1ockHashes11111111111111111111",
    "SysvarRewards111111111111111111111111111111",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RequestedAccountOverride {
    pub pubkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lamports: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
}

pub(in super::super::super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("INVALID_REQUESTED_INPUT", message)
}

pub(in super::super::super) fn bytes(encoded: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = STANDARD.decode(encoded).map_err(|_| invalid(label))?;
    if STANDARD.encode(&bytes) != encoded {
        return Err(invalid(label));
    }
    Ok(bytes)
}

pub(in super::super::super) fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    let b = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid("truncated u32"))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub(in super::super::super) fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    let b = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| invalid("truncated u64"))?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

pub fn validate_requested_overrides(value: &Value) -> Result<Vec<RequestedAccountOverride>> {
    let rows = value
        .as_array()
        .ok_or_else(|| invalid("requested account overrides missing or malformed"))?;
    if rows.is_empty() {
        return Err(invalid("EMPTY_ACCOUNT_OVERRIDES"));
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| invalid("account override missing or malformed"))?;
        if row
            .keys()
            .any(|key| !["pubkey", "lamports", "dataBase64"].contains(&key.as_str()))
        {
            return Err(invalid("FORBIDDEN_ACCOUNT_OVERRIDE_FIELD"));
        }
        let pubkey = row
            .get("pubkey")
            .and_then(Value::as_str)
            .filter(|key| !key.is_empty())
            .ok_or_else(|| invalid("INVALID_OVERRIDE_ACCOUNT"))?;
        if !seen.insert(pubkey.to_owned()) {
            return Err(invalid(format!("DUPLICATE_ACCOUNT_OVERRIDE:{pubkey}")));
        }
        if !row.contains_key("lamports") && !row.contains_key("dataBase64") {
            return Err(invalid(format!("EMPTY_ACCOUNT_OVERRIDE:{pubkey}")));
        }
        let lamports = match row.get("lamports") {
            None => None,
            Some(value) => {
                let text = value
                    .as_str()
                    .ok_or_else(|| invalid(format!("INVALID_OVERRIDE_LAMPORTS:{pubkey}")))?;
                if text.is_empty()
                    || (text != "0" && text.starts_with('0'))
                    || !text.bytes().all(|b| b.is_ascii_digit())
                    || text.parse::<u64>().is_err()
                {
                    return Err(invalid(format!("INVALID_OVERRIDE_LAMPORTS:{pubkey}")));
                }
                Some(text.to_owned())
            }
        };
        let data_base64 = match row.get("dataBase64") {
            None => None,
            Some(value) => {
                const MAX: usize = 10 * 1024 * 1024;
                let text = value
                    .as_str()
                    .ok_or_else(|| invalid(format!("INVALID_OVERRIDE_DATA:{pubkey}")))?;
                if text.len() > MAX.div_ceil(3) * 4
                    || bytes(text, &format!("INVALID_OVERRIDE_DATA:{pubkey}"))?.len() > MAX
                {
                    return Err(invalid(format!("INVALID_OVERRIDE_DATA:{pubkey}")));
                }
                Some(text.to_owned())
            }
        };
        result.push(RequestedAccountOverride {
            pubkey: pubkey.into(),
            lamports,
            data_base64,
        });
    }
    Ok(result)
}

pub(in super::super::super) fn checked_overrides(
    overrides: &[RequestedAccountOverride],
) -> Result<Vec<RequestedAccountOverride>> {
    if overrides.is_empty() {
        return Ok(Vec::new());
    }
    validate_requested_overrides(
        &serde_json::to_value(overrides).map_err(|e| invalid(e.to_string()))?,
    )
}

pub fn overridden_lookup_needs_slot_hashes(
    overrides: &[RequestedAccountOverride],
    table_keys: &[String],
    target_slot: u64,
) -> Result<bool> {
    super::super::super::safe_integer(target_slot, "target slot")?;
    for entry in overrides {
        if !table_keys.contains(&entry.pubkey) {
            continue;
        }
        let Some(encoded) = &entry.data_base64 else {
            continue;
        };
        let data = bytes(encoded, &format!("INVALID_ALT_OVERRIDE:{}", entry.pubkey))?;
        if data.len() < 56 || u32_at(&data, 0)? != 1 {
            return Err(invalid(format!("INVALID_ALT_OVERRIDE:{}", entry.pubkey)));
        }
        let deactivation = u64_at(&data, 4)?;
        if deactivation != u64::MAX && deactivation != target_slot {
            return Ok(true);
        }
    }
    Ok(false)
}
