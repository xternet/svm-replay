use super::*;

pub(in super::super) const DEDICATED: [(&str, &str); 5] = [
    (
        "sol_get_clock_sysvar",
        "SysvarC1ock11111111111111111111111111111111",
    ),
    (
        "sol_get_rent_sysvar",
        "SysvarRent111111111111111111111111111111111",
    ),
    (
        "sol_get_last_restart_slot",
        "SysvarLastRestartS1ot1111111111111111111111",
    ),
    (
        "sol_get_epoch_schedule_sysvar",
        "SysvarEpochSchedu1e111111111111111111111111",
    ),
    (
        "sol_get_epoch_rewards_sysvar",
        "SysvarEpochRewards1111111111111111111111111",
    ),
];

pub(in super::super) fn binary(
    program: &str,
    accounts: &[Value],
) -> Result<Option<(String, Vec<u8>)>, Error> {
    let mut matching = accounts.iter().filter(|a| a["pubkey"] == program);
    let Some(account) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err(Error::new(
            "HISTORICAL_ACCOUNT",
            "duplicate executable account",
        ));
    }
    if account["presence"] != "present" || account["executable"] != true {
        return Ok(None);
    }
    let key = program_data_address(account)?;
    let executable = match &key {
        None => account,
        Some(key) => {
            let mut found = accounts.iter().filter(|a| a["pubkey"] == *key);
            let data = found.next().ok_or_else(|| {
                Error::new("HISTORICAL_ACCOUNT", "executable ProgramData missing")
            })?;
            if found.next().is_some() || data["role"] != "programdata" {
                return Err(Error::new(
                    "HISTORICAL_ACCOUNT",
                    "ProgramData role/uniqueness mismatch",
                ));
            }
            data
        }
    };
    Ok(Some((
        key.unwrap_or_else(|| program.into()),
        account_data(executable)?,
    )))
}

pub(in super::super) fn imports(bytes: &[u8]) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    for start in 0..bytes.len().saturating_sub(7) {
        if !bytes[start..].starts_with(b"sol_get_") {
            continue;
        }
        let suffix = bytes[start + 8..]
            .iter()
            .take_while(|c| c.is_ascii_lowercase() || **c == b'_')
            .count();
        if suffix > 0 {
            values.insert(String::from_utf8_lossy(&bytes[start..start + 8 + suffix]).into_owned());
        }
    }
    values
}

pub fn dedicated_inputs(programs: &[String], accounts: &[Value]) -> Result<Vec<String>, Error> {
    let mut required = BTreeSet::new();
    for program in programs {
        if let Some((_, bytes)) = binary(program, accounts)? {
            let names = imports(&bytes);
            for (name, id) in &DEDICATED[3..] {
                if names.contains(*name) {
                    required.insert((*id).to_owned());
                }
            }
        }
    }
    Ok(required.into_iter().collect())
}

pub fn inspect(
    programs: &[String],
    accounts: &[Value],
    initialized: &[String],
    exact_epoch_stakes: bool,
) -> Result<Value, Error> {
    let mut scanned = Vec::new();
    let mut seen = BTreeSet::new();
    for program in programs {
        if !seen.insert(program) {
            continue;
        }
        if let Some((key, bytes)) = binary(program, accounts)? {
            let names = imports(&bytes);
            if names.contains("sol_get_epoch_stake") && !exact_epoch_stakes {
                return Err(Error::new("UNSUPPORTED_HISTORICAL_EPOCH_STAKE", program));
            }
            if names.contains("sol_get_fees_sysvar") {
                return Err(Error::new(
                    "UNSUPPORTED_HIDDEN_SYSVAR_INPUT",
                    format!("{program}:Fees"),
                ));
            }
            for (name, id) in &DEDICATED[3..] {
                if names.contains(*name) && !initialized.iter().any(|key| key == id) {
                    return Err(Error::new(
                        "UNSUPPORTED_HIDDEN_SYSVAR_INPUT",
                        format!("{program}:{id}"),
                    ));
                }
            }
            scanned.push(json!({"programId":program,"binaryAccount":key,"binarySha256":Digest::of(&bytes),"imports":names}));
        }
    }
    Ok(
        json!({"method":"executed-program-import-inventory-plus-guarded-generic-execution/v1",
        "programs":scanned,"genericReadsRequireGuard":true,"certifiedSimulation":false}),
    )
}
