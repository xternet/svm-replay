use super::*;

pub fn assert_requested_execution_shape(execution: &Value) -> Result<()> {
    if let Some(trace) = execution.get("instructionTrace") {
        assert_runtime_instruction_trace(trace)?;
    }
    match string(field(execution, "status")?)? {
        "ok" => check(
            field(execution, "normalizedError")?.is_null(),
            "requested execution error mismatch",
        )?,
        "err" => {
            string(field(execution, "normalizedError")?)?;
        }
        _ => {
            return Err(VerificationError(
                "requested execution status malformed".into(),
            ))
        }
    }
    strings(field(execution, "logs")?)?;
    integer(field(execution, "computeUnits")?)?;
    u64_string(field(execution, "fee")?)?;
    return_data(execution.get("returnData"), false)?;
    let mut transitions = BTreeSet::new();
    for row in array(field(execution, "accountTransitions")?)? {
        check(
            transitions.insert(string(field(row, "pubkey")?)?),
            "duplicate requested transition",
        )?;
        for key in ["before", "after"] {
            requested_state(field(row, key)?)?;
        }
    }
    let mut ends = BTreeSet::new();
    for row in array(field(execution, "endAccountStates")?)? {
        check(
            ends.insert(string(field(row, "pubkey")?)?),
            "duplicate requested end state",
        )?;
        requested_state(field(row, "state")?)?;
    }
    Ok(())
}

pub(in super::super) fn u64_string(value: &Value) -> Result<u64> {
    check(value.is_string(), "expected u64 string")?;
    unsigned(value)
}

pub(in super::super) fn requested_state(state: &Value) -> Result<()> {
    state_lamports(state)?;
    if field(state, "presence")? == "present" {
        u64_string(field(state, "lamports")?)?;
        u64_string(field(state, "rentEpoch")?)?;
        if !field(state, "tokenAmount")?.is_null() {
            u64_string(field(state, "tokenAmount")?)?;
        }
    }
    Ok(())
}

pub(in super::super) fn require_trace(execution: &Value, required: bool) -> Result<()> {
    check(
        !required || object(execution)?.contains_key("instructionTrace"),
        "required instruction trace missing",
    )
}

pub(in super::super) fn execution_behavior(value: &Value) -> Result<Value> {
    let mut result = Map::new();
    for key in [
        "status",
        "normalizedError",
        "logs",
        "computeUnits",
        "returnData",
        "accountTransitions",
    ] {
        result.insert(key.into(), field(value, key)?.clone());
    }
    Ok(Value::Object(result))
}

pub(in super::super) fn nullable_object(value: &Value) -> Result<Option<&Value>> {
    if value.is_null() {
        Ok(None)
    } else {
        object(value)?;
        Ok(Some(value))
    }
}

pub(in super::super) fn requested_override_proof(
    fixture: &Value,
    execution: &Value,
) -> Result<Value> {
    let overrides = array(field(fixture, "requestedAccountOverrides")?)?;
    check(!overrides.is_empty(), "EMPTY_ACCOUNT_OVERRIDES")?;
    let transitions = array(field(execution, "accountTransitions")?)?;
    let mut seen = BTreeSet::new();
    for entry in overrides {
        check(
            object(entry)?
                .keys()
                .all(|key| ["pubkey", "lamports", "dataBase64"].contains(&key.as_str())),
            "FORBIDDEN_ACCOUNT_OVERRIDE_FIELD",
        )?;
        let pubkey = string(field(entry, "pubkey")?)?;
        check(seen.insert(pubkey), "DUPLICATE_ACCOUNT_OVERRIDE")?;
        let lamports = entry.get("lamports").map(u64_string).transpose()?;
        let data = entry.get("dataBase64").map(base64_bytes).transpose()?;
        check(
            lamports.is_some() || data.is_some(),
            "EMPTY_ACCOUNT_OVERRIDE",
        )?;
        if let Some(data) = &data {
            check(data.len() <= 10 * 1024 * 1024, "INVALID_OVERRIDE_DATA")?;
        }
        let mut matched = None;
        for row in transitions {
            if string(field(row, "pubkey")?)? == pubkey {
                matched = Some(row);
            }
        }
        let row = matched.ok_or_else(|| {
            VerificationError(format!("requested override transition missing: {pubkey}"))
        })?;
        let before = field(row, "before")?;
        if let Some(lamports) = lamports {
            check(
                state_lamports(before)? == lamports,
                "requested override lamports mismatch",
            )?;
        }
        if let Some(data) = data {
            if lamports != Some(0) {
                equal(
                    field(before, "dataHash")?,
                    &json!(hash(data)),
                    "requested override data",
                )?;
            }
        }
    }
    let target = field(fixture, "target")?;
    let replacement = field(target, "replacementTransactionBase64")?;
    let payload = if replacement.is_null() {
        field(target, "transactionBase64")?
    } else {
        replacement
    };
    Ok(
        json!({"attempted":true,"accounts":overrides.len(),"payloadSha256":hash(base64_bytes(payload)?),
        "overridesSha256":hash(canonical_json(field(fixture, "requestedAccountOverrides")?))}),
    )
}
