use super::*;

pub const CLOCK_SYSVAR: &str = "SysvarC1ock11111111111111111111111111111111";

pub const SYSVAR_OWNER: &str = "Sysvar1111111111111111111111111111111111111";

pub const GENERIC_SYSVARS: [&str; 7] = [
    CLOCK_SYSVAR,
    "SysvarEpochSchedu1e111111111111111111111111",
    "SysvarEpochRewards1111111111111111111111111",
    "SysvarRent111111111111111111111111111111111",
    "SysvarS1otHashes111111111111111111111111111",
    "SysvarStakeHistory1111111111111111111111111",
    "SysvarLastRestartS1ot1111111111111111111111",
];

pub const BANK_INITIALIZED_SYSVARS: [&str; 5] = [
    "SysvarRent111111111111111111111111111111111",
    "SysvarEpochRewards1111111111111111111111111",
    "SysvarEpochSchedu1e111111111111111111111111",
    "SysvarStakeHistory1111111111111111111111111",
    "SysvarLastRestartS1ot1111111111111111111111",
];

pub(in super::super) fn target_clock_bytes(account: &Value, target_slot: u64) -> Result<Vec<u8>> {
    let data = bytes(field(account, "dataBase64")?)?;
    check(data.len() == 40, "Clock must have exactly 40 bytes")?;
    let embedded = u64::from_le_bytes(
        data[..8]
            .try_into()
            .map_err(|e| fail(format!("Clock bytes: {e}")))?,
    );
    check(
        embedded == target_slot,
        "Clock payload does not match target slot",
    )?;
    Ok(data)
}

/// Convert an exact-slot RPC account row into the strict target Clock image.
pub fn build_target_clock(account: &Value, target_slot: u64) -> Result<Value> {
    check(
        field(account, "pubkey")? == CLOCK_SYSVAR,
        "target Clock pubkey mismatch",
    )?;
    let value = field(account, "value")?;
    check(!value.is_null(), "Clock absent at target slot")?;
    let data = array(field(value, "data")?)?;
    check(
        data.len() == 2 && data[1] == "base64",
        "malformed Clock data encoding",
    )?;
    bytes(&data[0])?;
    check(
        field(value, "executable")?.is_boolean(),
        "Clock executable malformed",
    )?;
    pubkey(field(value, "owner")?)?;
    let result = json!({"pubkey":CLOCK_SYSVAR,"sourceSlot":target_slot,"role":"sysvar","presence":"present",
        "lamports":uint(field(value,"lamports")?)?.to_string(),"owner":field(value,"owner")?,"executable":field(value,"executable")?,
        "rentEpoch":uint(field(value,"rentEpoch")?)?.to_string(),"dataBase64":data[0]});
    target_clock_bytes(&result, target_slot)?;
    Ok(result)
}

/// Validate an already classified strict image without projecting it back into RPC JSON.
pub fn validate_target_clock(account: &Value, target_slot: u64) -> Result<()> {
    check(
        field(account, "pubkey")? == CLOCK_SYSVAR,
        "target Clock pubkey mismatch",
    )?;
    bind_exact_generic_sysvar(account, target_slot)?;
    Ok(())
}

pub fn bind_exact_generic_sysvar(account: &Value, slot: u64) -> Result<Value> {
    let id = string(field(account, "pubkey")?)?;
    check(
        GENERIC_SYSVARS.contains(&id)
            && field(account, "sourceSlot")? == &json!(slot)
            && field(account, "role")? == "sysvar"
            && field(account, "presence")? == "present"
            && field(account, "owner")? == SYSVAR_OWNER
            && field(account, "executable")? == &json!(false),
        &format!("exact generic sysvar unavailable:{id}"),
    )?;
    let data = if id == CLOCK_SYSVAR {
        target_clock_bytes(account, slot)?
    } else {
        bytes(field(account, "dataBase64")?)?
    };
    Ok(json!({"pubkey":id,"sourceSlot":slot,"dataSha256":hash(&data),"dataLen":data.len()}))
}

pub fn assert_bank_initialized_sysvar_bindings(
    accounts: &[Value],
    slot: u64,
    bindings: &[Value],
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for binding in bindings {
        let id = string(field(binding, "pubkey")?)?;
        check(seen.insert(id), "duplicate initialized sysvar binding")?;
        let mut matching = accounts
            .iter()
            .filter(|a| a.get("pubkey") == Some(&json!(id)));
        let account = matching
            .next()
            .ok_or_else(|| fail("initialized sysvar account missing"))?;
        check(
            matching.next().is_none(),
            "duplicate initialized sysvar account",
        )?;
        check(
            BANK_INITIALIZED_SYSVARS.contains(&id)
                && field(binding, "phasePolicy")? == "reserved-exempt-bank-initialized-sysvar/v1"
                && integer(field(binding, "parentSlot")?)? < slot,
            "initialized sysvar phase/boundary mismatch",
        )?;
        digest(string(field(binding, "parentAccountSha256")?)?)?;
        check(
            field(binding, "targetAccountSha256")? == &json!(hash(canonical_json(account))),
            "initialized sysvar target image mismatch",
        )?;
        let exact = bind_exact_generic_sysvar(account, slot)?;
        for key in ["sourceSlot", "dataSha256", "dataLen"] {
            check(
                field(binding, key)? == field(&exact, key)?,
                "initialized sysvar bytes/binding mismatch",
            )?;
        }
    }
    Ok(())
}

/// Apply caller-supplied exact target images only for proven parent-exempt reserved sysvars.
pub fn bind_bank_initialized_sysvars(
    parent: &[Value],
    target: &[Value],
    slot: u64,
) -> Result<Value> {
    let mut replacements = BTreeMap::new();
    let mut bindings = Vec::new();
    for before in parent {
        let id = string(field(before, "pubkey")?)?;
        if !BANK_INITIALIZED_SYSVARS.contains(&id) {
            continue;
        }
        check(
            !replacements.contains_key(id),
            "duplicate parent initialized sysvar",
        )?;
        let mut matching = target
            .iter()
            .filter(|a| a.get("pubkey") == Some(&json!(id)));
        let after = matching
            .next()
            .ok_or_else(|| fail(format!("exact target sysvar unavailable:{id}")))?;
        check(matching.next().is_none(), "duplicate exact target sysvar")?;
        check(
            integer(field(before, "sourceSlot")?)? < slot
                && field(before, "presence")? == "present"
                && field(before, "role")? == "sysvar"
                && field(before, "owner")? == SYSVAR_OWNER
                && field(before, "executable")? == &json!(false)
                && field(before, "rentEpoch")? == &json!(u64::MAX.to_string())
                && field(after, "rentEpoch")? == field(before, "rentEpoch")?,
            &format!("unproven initialized sysvar phase:{id}"),
        )?;
        let mut binding = bind_exact_generic_sysvar(after, slot)?;
        binding["parentSlot"] = field(before, "sourceSlot")?.clone();
        binding["parentAccountSha256"] = json!(hash(canonical_json(before)));
        binding["targetAccountSha256"] = json!(hash(canonical_json(after)));
        binding["phasePolicy"] = json!("reserved-exempt-bank-initialized-sysvar/v1");
        bindings.push(binding);
        replacements.insert(id, after);
    }
    let accounts: Vec<Value> = parent
        .iter()
        .map(|account| {
            let id = string(field(account, "pubkey")?)?;
            Ok(match replacements.get(id) {
                Some(replacement) => (*replacement).clone(),
                None => account.clone(),
            })
        })
        .collect::<Result<_>>()?;
    assert_bank_initialized_sysvar_bindings(&accounts, slot, &bindings)?;
    Ok(json!({"accounts":accounts,"bindings":bindings}))
}
