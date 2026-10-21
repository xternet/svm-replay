use super::*;

/// Checks the worker's `whatIf` output against the fixture's request. Returns the proof summary
/// that joins the verification basis, or None when neither side has a what-if run.
pub(in super::super) fn what_if_proof(
    fixture: &Value,
    output: &Value,
    trace_required: bool,
) -> Result<Option<Value>> {
    let actual = match output.get("whatIf") {
        None => None,
        Some(value) => nullable_object(value)?.map(|_| value),
    };
    match (fixture.get("whatIf"), actual) {
        (None, None) => Ok(None),
        (Some(expected), Some(actual)) => proof(expected, actual, trace_required).map(Some),
        _ => Err(VerificationError("what-if attempt mismatch".into())),
    }
}

fn proof(expected: &Value, actual: &Value, trace_required: bool) -> Result<Value> {
    let mode = string(field(expected, "mode")?)?;
    check(
        ["after", "replace"].contains(&mode),
        "what-if mode malformed",
    )?;
    equal(field(actual, "mode")?, &json!(mode), "what-if mode")?;
    let requested = array(field(expected, "transactionsBase64")?)?;
    let transactions = array(field(actual, "transactions")?)?;
    check(
        !requested.is_empty() && transactions.len() == requested.len(),
        "what-if transaction count mismatch",
    )?;
    for execution in transactions {
        require_trace(execution, trace_required)?;
        assert_requested_execution_shape(execution)?;
    }
    let overrides = match expected.get("stateOverrides") {
        None => &[][..],
        Some(value) => array(value)?.as_slice(),
    };
    let applied = array(field(actual, "stateOverrides")?)?;
    check(
        applied.len() == overrides.len(),
        "state override count mismatch",
    )?;
    for (request, row) in overrides.iter().zip(applied) {
        let pubkey = string(field(request, "pubkey")?)?;
        equal(
            field(row, "pubkey")?,
            &json!(pubkey),
            "state override order",
        )?;
        requested_state(field(row, "before")?)?;
        let after = field(row, "after")?;
        requested_state(after)?;
        applied_fields(request, after)
            .map_err(|error| VerificationError(format!("state override {pubkey}: {error}")))?;
    }
    Ok(json!({"mode":mode,"transactions":transactions.len(),"stateOverridesApplied":true}))
}

fn applied_fields(request: &Value, after: &Value) -> Result<()> {
    if request.get("lamports") == Some(&json!("0")) {
        return check(
            field(after, "presence")? == "absent",
            "account still present",
        );
    }
    check(field(after, "presence")? == "present", "account absent")?;
    for name in ["lamports", "owner", "executable", "rentEpoch"] {
        if let Some(value) = request.get(name) {
            equal(field(after, name)?, value, name)?;
        }
    }
    if let Some(data) = request.get("dataBase64") {
        equal(
            field(after, "dataHash")?,
            &json!(hash(base64_bytes(data)?)),
            "data",
        )?;
    }
    Ok(())
}
