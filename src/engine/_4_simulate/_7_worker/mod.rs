use super::*;

pub struct Execution {
    pub output: Value,
    pub reads: Value,
    pub process: Value,
}

pub fn run(
    transport: &WorkerTransport,
    spec: &WorkerSpec,
    fixture: &Value,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
) -> Result<Execution, Error> {
    let available = context::available_generic_sysvars(fixture)?;
    let execution = transport
        .run(
            spec,
            fixture,
            &["--discover-sysvars".into()],
            limits,
            budget,
        )
        .map_err(worker_error)?;
    let value = execution.value;
    if value.get("schema").and_then(Value::as_str) == Some("svm-epoch-stake-input-failure/v1") {
        context::validate_epoch_stake_failure(&value, fixture)?;
        return Err(Error::new(
            "UNSUPPORTED_HISTORICAL_EPOCH_STAKE",
            "exact epoch-stake input unavailable",
        )
        .with_details(value));
    }
    let value = context::parse_sysvar_discovery_response(&value, &available)?;
    if value.get("schema").and_then(Value::as_str) != Some("svm-sysvar-discovery/v1") {
        return Err(Error::new("WORKER_PROTOCOL", "unknown guarded result"));
    }
    if value.get("status").and_then(Value::as_str) == Some("NEEDS_INPUT")
        && value.get("output").is_none()
        && value.get("reads").is_some_and(Value::is_array)
    {
        return Err(
            Error::new("NEEDS_INPUT", "exact historical runtime input unavailable")
                .with_details(value),
        );
    }
    if value.get("status").and_then(Value::as_str) != Some("COMPLETE")
        || !value.get("reads").is_some_and(Value::is_array)
    {
        return Err(Error::new(
            "WORKER_PROTOCOL",
            "incomplete or malformed guarded output",
        ));
    }
    let output = value
        .get("output")
        .ok_or_else(|| Error::new("WORKER_PROTOCOL", "missing guarded output"))?
        .clone();
    Ok(Execution {
        output,
        reads: value["reads"].clone(),
        process: json!({"pid":execution.pid,"elapsedMs":execution.elapsed.as_millis(),
        "stdout":String::from_utf8_lossy(&execution.stdout),"stderr":String::from_utf8_lossy(&execution.stderr),"cleanup":format!("{:?}",execution.cleanup_scope)}),
    })
}
