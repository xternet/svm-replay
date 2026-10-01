use super::*;

pub(in super::super) const TRACKED: &str = "tracked-generic-sysvar-context-v1";

pub(in super::super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("SYSVAR_DISCOVERY_PROTOCOL", message)
}

pub(in super::super) fn target_slot(fixture: &Value) -> Result<u64, Error> {
    fixture["target"]["targetSlot"]
        .as_u64()
        .ok_or_else(|| invalid("missing target slot"))
}

fn input_binding(input: &Value, slot: u64) -> Result<Value, Error> {
    if input.get("schema").is_some() {
        slot_hashes::bind_data(input, slot)
    } else {
        bind_exact_generic_sysvar(input, slot)
    }
}

pub fn initial_inputs(base: &Value) -> Result<Vec<Value>, Error> {
    let slot = target_slot(base)?;
    let clock = base
        .get("clock")
        .ok_or_else(|| invalid("exact initial Clock required"))?;
    bind_exact_generic_sysvar(clock, slot)?;
    let mut inputs = vec![clock.clone()];
    let mut seen = BTreeSet::new();
    if let Some(bindings) = base["runtime"].get("genericSysvars") {
        for binding in bindings
            .as_array()
            .ok_or_else(|| invalid("generic bindings must be array"))?
        {
            let key = binding["pubkey"]
                .as_str()
                .ok_or_else(|| invalid("binding pubkey"))?;
            if !seen.insert(key.to_owned()) {
                return Err(invalid("duplicate initial binding"));
            }
            let values = if key == CLOCK {
                vec![clock]
            } else if key == slot_hashes::SLOT_HASHES
                && base["runtime"].get("slotHashesData").is_some()
            {
                slot_hashes::bind_fixture(base)?;
                vec![&base["runtime"]["slotHashesData"]]
            } else {
                base["accounts"]
                    .as_array()
                    .ok_or_else(|| invalid("accounts missing"))?
                    .iter()
                    .filter(|a| a["pubkey"] == key)
                    .collect()
            };
            if values.len() != 1 || input_binding(values[0], slot)? != *binding {
                return Err(invalid("initial generic binding differs"));
            }
            if key != CLOCK {
                inputs.push(values[0].clone());
            }
        }
    }
    for required in base["runtime"]["bankContext"]["requiredRuntimeSysvars"]
        .as_array()
        .ok_or_else(|| invalid("runtime requirements missing"))?
    {
        if required["requirement"] == TRACKED
            && !inputs.iter().any(|a| a["pubkey"] == required["pubkey"])
        {
            return Err(invalid("tracked input lacks proven bytes"));
        }
    }
    Ok(inputs)
}

pub fn add_input(
    inputs: &mut Vec<Value>,
    key: &str,
    account: Value,
    slot: u64,
) -> Result<(), Error> {
    if !GENERIC_SYSVARS.contains(&key)
        || inputs.iter().any(|a| a["pubkey"] == key)
        || inputs.len() >= GENERIC_SYSVARS.len()
    {
        return Err(invalid("discovery cannot make proven progress"));
    }
    if account["pubkey"] != key {
        return Err(invalid("hydration returned wrong account"));
    }
    input_binding(&account, slot)?;
    inputs.push(account);
    Ok(())
}

pub fn tracked_fixture(base: &Value, inputs: &[Value]) -> Result<Value, Error> {
    let slot = target_slot(base)?;
    let mut fixture = base.clone();
    let requirements = base["runtime"]["bankContext"]["requiredRuntimeSysvars"]
        .as_array()
        .ok_or_else(|| invalid("runtime requirements missing"))?;
    if requirements
        .iter()
        .any(|v| v["requirement"] == "complete-generic-sysvar-context-v1")
    {
        return Err(invalid("complete and tracked modes cannot be mixed"));
    }
    let mut keys = BTreeSet::new();
    let mut bindings = Vec::new();
    for account in inputs {
        let key = account["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("missing sysvar pubkey"))?;
        if !keys.insert(key.to_owned()) {
            return Err(invalid("duplicate exact input"));
        }
        bindings.push(input_binding(account, slot)?);
        if account.get("schema").is_some() {
            fixture["runtime"]["slotHashesData"] = account.clone();
            slot_hashes::bind_fixture(&fixture)?;
        }
    }
    let clock = inputs
        .iter()
        .find(|a| a["pubkey"] == CLOCK)
        .ok_or_else(|| invalid("exact initial Clock required"))?;
    let mut accounts = Vec::new();
    for account in base["accounts"]
        .as_array()
        .ok_or_else(|| invalid("accounts missing"))?
    {
        let key = account["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("account pubkey missing"))?;
        if !keys.contains(key) {
            accounts.push(account.clone());
        }
    }
    accounts.extend(
        inputs
            .iter()
            .filter(|a| a["pubkey"] != CLOCK && a.get("schema").is_none())
            .cloned(),
    );
    fixture["accounts"] = json!(accounts);
    fixture["clock"] = clock.clone();
    if let Some(bindings) = fixture["runtime"].get("bankInitializedSysvars") {
        assert_bank_initialized_sysvar_bindings(
            &accounts,
            slot,
            bindings
                .as_array()
                .ok_or_else(|| invalid("initialized bindings must be array"))?,
        )?;
    }
    context::assert_bound_context(&fixture)?;
    let mut required = requirements
        .iter()
        .filter(|v| v["requirement"] != TRACKED)
        .cloned()
        .collect::<Vec<_>>();
    required.extend(
        bindings
            .iter()
            .map(|b| json!({"pubkey":b["pubkey"],"requirement":TRACKED})),
    );
    fixture["runtime"]["genericSysvars"] = json!(bindings);
    fixture["runtime"]["bankContext"]["requiredRuntimeSysvars"] = json!(required);
    fixture["runtime"]["bankContext"]["derivationHash"] = json!(historical_bank_context_hash(
        &fixture["runtime"]["bankContext"]
    )?);
    fixture["runtime"]["clockDataHash"] =
        bind_exact_generic_sysvar(clock, slot)?["dataSha256"].clone();
    fixture["runtime"]["profileHash"] = json!(runtime_profile_hash(&fixture["runtime"])?);
    context::available_generic_sysvars(&fixture)?;
    Ok(fixture)
}

pub fn runtime_profile_hash(runtime: &Value) -> Result<Digest, Error> {
    let required = |pointer: &str| {
        runtime
            .pointer(pointer)
            .cloned()
            .ok_or_else(|| invalid(format!("profile missing {pointer}")))
    };
    let mut basis = json!({"bindingHash":required("/binding/bindingHash")?,"bankContextHash":required("/bankContext/derivationHash")?,
        "clockDataHash":required("/clockDataHash")?,"lastRestartSlotBindingHash":required("/lastRestartSlot/bindingHash")?});
    for name in [
        "genericSysvars",
        "slotHashesData",
        "bankInitializedSysvars",
        "programMigrationProofs",
        "programHeaderProofs",
        "epochStakes",
        "initializedStakeSnapshot",
        "recentAddressTables",
        "stakeInitializationProofs",
        "systemCreditPreloads",
    ] {
        if let Some(value) = runtime.get(name) {
            basis[name] = value.clone();
        }
    }
    Ok(Digest::of(format!(
        "m6-runtime-profile/v1\n{}\n",
        canonical_json(&basis)
    )))
}

pub struct Settled {
    pub fixture: Value,
    pub output: Value,
    pub attempts: Vec<Value>,
}
