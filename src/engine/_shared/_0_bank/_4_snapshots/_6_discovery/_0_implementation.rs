use super::*;

pub const GENERIC_SYSVARS: [&str; 7] = [
    "SysvarC1ock11111111111111111111111111111111",
    "SysvarEpochSchedu1e111111111111111111111111",
    "SysvarEpochRewards1111111111111111111111111",
    "SysvarRent111111111111111111111111111111111",
    "SysvarS1otHashes111111111111111111111111111",
    "SysvarStakeHistory1111111111111111111111111",
    "SysvarLastRestartS1ot1111111111111111111111",
];

pub(in super::super) fn generic_binding(fixture: &Value, id: &str) -> Check<Value> {
    require(GENERIC_SYSVARS.contains(&id), "unknown generic sysvar")?;
    if id == GENERIC_SYSVARS[4] && fixture["runtime"].get("slotHashesData").is_some() {
        return crate::shared::bank::slot_hashes::bind_fixture(fixture).map_err(|e| e.to_string());
    }
    let target_slot = slot(field(field(fixture, "target")?, "targetSlot")?)?;
    let account = if id == GENERIC_SYSVARS[0] {
        field(fixture, "clock")?
    } else {
        let accounts = array(field(fixture, "accounts")?)?;
        let mut matching = accounts
            .iter()
            .filter(|account| account.get("pubkey") == Some(&json!(id)));
        let account = matching
            .next()
            .ok_or_else(|| format!("initial generic account missing:{id}"))?;
        require(
            matching.next().is_none(),
            &format!("duplicate initial generic account:{id}"),
        )?;
        account
    };
    require(
        field(account, "pubkey")? == id
            && field(account, "sourceSlot")? == &json!(target_slot)
            && field(account, "role")? == "sysvar"
            && field(account, "presence")? == "present"
            && field(account, "owner")? == "Sysvar1111111111111111111111111111111111111"
            && field(account, "executable")? == &json!(false),
        &format!("exact generic sysvar slot/shape mismatch:{id}"),
    )?;
    let data = if id == GENERIC_SYSVARS[0] {
        clock_data(account, target_slot)?
    } else {
        bytes(field(account, "dataBase64")?)?
    };
    Ok(
        json!({"pubkey":id,"sourceSlot":target_slot,"dataSha256":sha256(&data),"dataLen":data.len()}),
    )
}

pub(in super::super) fn generic_available(fixture: &Value) -> Check<BTreeSet<String>> {
    let runtime = field(fixture, "runtime")?;
    let requirements = array(field(
        field(runtime, "bankContext")?,
        "requiredRuntimeSysvars",
    )?)?;
    let mut complete = BTreeSet::new();
    let mut tracked = BTreeSet::new();
    let mut nonce = false;
    for requirement in requirements {
        let id = pubkey(field(requirement, "pubkey")?)?;
        match string(field(requirement, "requirement")?)? {
            "tracked-generic-sysvar-context-v1" => {
                require(
                    GENERIC_SYSVARS.contains(&id) && tracked.insert(id.to_owned()),
                    "unknown/duplicate tracked generic requirement",
                )?;
            }
            "complete-generic-sysvar-context-v1" => {
                require(
                    GENERIC_SYSVARS.contains(&id) && complete.insert(id.to_owned()),
                    "unknown/duplicate complete generic requirement",
                )?;
            }
            "nonce-advance-nonempty-cache-v1" => {
                require(
                    id == "SysvarRecentB1ockHashes11111111111111111111" && !nonce,
                    "invalid/duplicate nonce requirement",
                )?;
                nonce = true;
            }
            _ => return Err("unknown historical Bank runtime sysvar requirement".into()),
        }
    }
    require(
        complete.is_empty() || complete.len() == GENERIC_SYSVARS.len(),
        "incomplete complete generic mode",
    )?;
    require(
        complete.is_empty() || tracked.is_empty(),
        "complete and tracked generic modes cannot be mixed",
    )?;
    require(
        tracked.is_empty() || tracked.contains(GENERIC_SYSVARS[0]),
        "tracked mode requires exact initial Clock",
    )?;
    let available: BTreeSet<String> = complete.into_iter().chain(tracked).collect();
    require(
        runtime.get("slotHashesData").is_none() || available.contains(GENERIC_SYSVARS[4]),
        "data-only SlotHashes is not declared to the guard",
    )?;
    for id in &available {
        generic_binding(fixture, id)?;
    }
    let mut bound = BTreeSet::new();
    if let Some(bindings) = runtime.get("genericSysvars") {
        for binding in array(bindings)? {
            shape(binding, &["pubkey", "sourceSlot", "dataSha256", "dataLen"])?;
            let id = pubkey(field(binding, "pubkey")?)?;
            require(
                bound.insert(id),
                &format!("duplicate initial generic binding:{id}"),
            )?;
            equal(
                binding,
                &generic_binding(fixture, id)?,
                &format!("initial generic binding:{id}"),
            )?;
        }
    }
    // The initial Clock is independently bound by its exact target payload.
    // Other declared inputs need the preparation binding, not merely account bytes.
    for id in &available {
        require(
            id == GENERIC_SYSVARS[0] || bound.contains(id.as_str()),
            &format!("initial generic input has no bound bytes:{id}"),
        )?;
    }
    Ok(available)
}

/// The exact set passed to the worker guard, without implicitly adding or fetching inputs.
pub fn available_generic_sysvars(fixture: &Value) -> Result<BTreeSet<String>, Error> {
    generic_available(fixture).map_err(|error| Error::new("INVALID_GENERIC_SYSVAR_INPUT", error))
}

pub(in super::super) fn discovery_response(
    value: &Value,
    available: &BTreeSet<String>,
) -> Check<Value> {
    require(
        field(value, "schema")? == "svm-sysvar-discovery/v1",
        "invalid sysvar discovery schema",
    )?;
    let status = string(field(value, "status")?)?;
    require(
        status == "NEEDS_INPUT" || status == "COMPLETE",
        "invalid sysvar discovery status",
    )?;
    shape(
        value,
        &[
            "schema",
            "status",
            "reads",
            if status == "NEEDS_INPUT" {
                "pubkey"
            } else {
                "output"
            },
        ],
    )?;
    let reads = array(field(value, "reads")?)?;
    for (index, read) in reads.iter().enumerate() {
        let validate = || -> Check<()> {
            shape(read, &["pubkey", "offset", "length"])?;
            pubkey(field(read, "pubkey")?)?;
            u64_string(field(read, "offset")?)?;
            u64_string(field(read, "length")?)?;
            Ok(())
        };
        validate().map_err(|error| format!("sysvar read {index}: {error}"))?;
    }
    if status == "NEEDS_INPUT" {
        let id = pubkey(field(value, "pubkey")?)?;
        require(
            GENERIC_SYSVARS.contains(&id)
                && !available.contains(id)
                && reads
                    .iter()
                    .any(|read| read.get("pubkey") == Some(&json!(id))),
            &format!("sysvar discovery cannot make proven progress:{id}"),
        )?;
    } else {
        for read in reads {
            let id = string(field(read, "pubkey")?)?;
            require(
                !GENERIC_SYSVARS.contains(&id) || available.contains(id),
                &format!("COMPLETE includes unproven generic read:{id}"),
            )?;
        }
    }
    // COMPLETE's execution payload is deliberately not certified here. The
    // archived verifier must independently validate it before publication.
    Ok(value.clone())
}

/// Parse quarantined discovery responses; NEEDS_INPUT may never carry partial output.
pub fn parse_sysvar_discovery_response(
    value: &Value,
    available: &BTreeSet<String>,
) -> Result<Value, Error> {
    discovery_response(value, available)
        .map_err(|error| Error::new("SYSVAR_DISCOVERY_PROTOCOL_ERROR", error))
}
