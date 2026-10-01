use super::*;

pub(in super::super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("ALT_INPUT", message)
}

pub(in super::super) fn key(account: &Value) -> Result<&str> {
    account["pubkey"]
        .as_str()
        .ok_or_else(|| invalid("address table pubkey missing"))
}

pub(in super::super) fn bytes(account: &Value) -> Result<Vec<u8>> {
    let id = key(account)?;
    if account["presence"] != "present"
        || account["owner"] != ALT
        || account["executable"] != false
        || !account["dataBase64"].is_string()
    {
        return Err(invalid(format!("{id}: malformed historical address table")));
    }
    let bytes = account_data(account)
        .map_err(|_| invalid(format!("{id}: invalid address-table state encoding")))?;
    if bytes.len() < 56 || bytes[..4] != 1u32.to_le_bytes() {
        return Err(invalid(format!(
            "{id}: invalid address-table state encoding"
        )));
    }
    Ok(bytes)
}

pub(in super::super) fn deactivation(bytes: &[u8]) -> u64 {
    u64::from_le_bytes([
        bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11],
    ])
}

pub fn preflight_active_address_tables(accounts: &[Value]) -> Result<Value> {
    let mut seen = BTreeSet::new();
    let mut deactivated = Vec::new();
    for a in accounts
        .iter()
        .filter(|a| a["role"] == "address-lookup-table")
    {
        let id = key(a)?;
        if !seen.insert(id.to_owned()) {
            return Err(invalid(format!("duplicate address table {id}")));
        }
        let slot = deactivation(&bytes(a)?);
        if slot != u64::MAX {
            deactivated.push(json!({"pubkey":id,"deactivationSlot":slot.to_string()}));
        }
    }
    if seen.is_empty() {
        return Err(invalid("M2 address-table evidence missing"));
    }
    if !deactivated.is_empty() {
        // Prototype localeCompare orders base58 keys case-insensitively first,
        // then lowercase before uppercase. The active hash uses default sort.
        deactivated.sort_by(|a, b| {
            let a = a["pubkey"].as_str().expect("validated table key");
            let b = b["pubkey"].as_str().expect("validated table key");
            a.to_ascii_lowercase()
                .cmp(&b.to_ascii_lowercase())
                .then_with(|| {
                    a.bytes()
                        .map(|c| c.is_ascii_uppercase())
                        .cmp(b.bytes().map(|c| c.is_ascii_uppercase()))
                })
        });
        return Ok(json!({"status":"REQUIRES_SLOT_HASHES","tables":deactivated}));
    }
    let mut ids: Vec<_> = seen.into_iter().collect();
    ids.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    Ok(
        json!({"status":"NOT_REQUIRED","count":ids.len(),"pubkeysHash":Digest::of(format!("{}\n",ids.join("\n")).as_bytes()),"deactivationSlot":u64::MAX.to_string()}),
    )
}

pub fn build_recent_address_table_proofs(
    accounts: &[Value],
    target_slot: u64,
    parent_slot: u64,
    has_exact_slot_hashes: bool,
) -> Result<Vec<Value>> {
    let reject = |reason: String| unsupported("UNSUPPORTED_ALT_LIFECYCLE", "PREFLIGHT", reason);
    if target_slot > 9_007_199_254_740_991 || parent_slot >= target_slot {
        return Err(reject("invalid historical ALT boundary".into()));
    }
    let mut seen = BTreeSet::new();
    let mut proofs = Vec::new();
    for a in accounts
        .iter()
        .filter(|a| a["role"] == "address-lookup-table")
    {
        let id = key(a)?;
        if !seen.insert(id) {
            return Err(reject(format!("duplicate ALT {id}")));
        }
        if a["sourceSlot"].as_u64() != Some(parent_slot)
            || a["presence"] != "present"
            || a["owner"] != ALT
            || a["executable"] != false
            || !a["dataBase64"].is_string()
        {
            return Err(reject(format!("invalid canonical parent ALT {id}")));
        }
        let data = bytes(a).map_err(|_| reject(format!("invalid ALT data {id}")))?;
        if (data.len() - 56) % 32 != 0 || data.len() > 56 + 256 * 32 {
            return Err(reject(format!("invalid ALT data {id}")));
        }
        let slot = deactivation(&data);
        if slot == u64::MAX {
            continue;
        }
        if slot > parent_slot {
            return Err(reject(format!("invalid parent ALT deactivation {id}")));
        }
        if has_exact_slot_hashes {
            continue;
        }
        if target_slot - slot >= 512 {
            return Err(reject(format!(
                "ALT {id} requires historical ancestor membership"
            )));
        }
        proofs.push(json!({"pubkey":id,"dataSha256":Digest::of(&data),"deactivationSlot":slot.to_string(),"targetSlot":target_slot}));
    }
    Ok(proofs)
}
