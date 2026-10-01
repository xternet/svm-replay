use super::*;

pub(in super::super) fn checkpoint_run(
    fixture: &Value,
    mode: CheckpointMode,
    phase: &str,
    expected: Option<&Digest>,
    context: &CacheContext<'_>,
    worker: &ResolvedWorker,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    checkpoint_path: &std::path::PathBuf,
    prefix_count: usize,
    events: &mut Vec<Value>,
    totals: &mut Metrics,
) -> Result<boundary::CheckpointResponse, Error> {
    budget.check().map_err(protocol_error)?;
    let available = _2_prepare_state::context::available_generic_sysvars(fixture)?;
    let mut flags = vec![
        match mode {
            CheckpointMode::Create => "--checkpoint-create".into(),
            CheckpointMode::Run => "--checkpoint-run".into(),
        },
        checkpoint_path.display().to_string(),
    ];
    if let Some(hash) = expected {
        flags.push(hash.as_str().into());
    }
    let execution = context
        .transport
        .run_with_files(
            &worker.spec,
            fixture,
            &flags,
            std::slice::from_ref(&checkpoint_path),
            limits,
            budget,
        )
        .map_err(protocol_error)?;
    let response = execution.value;
    events.push(json!({"phase":phase,"mode":match mode {CheckpointMode::Create=>"create",CheckpointMode::Run=>"run"},"response":response_receipt(&response)?,
                "process":{"pid":execution.pid,"elapsedMs":execution.elapsed.as_millis(),"stdoutBase64":STANDARD.encode(execution.stdout),"stderrBase64":STANDARD.encode(execution.stderr),"cleanup":format!("{:?}",execution.cleanup_scope)}}));
    if response["schema"] != "svm-m10-checkpoint-output/v1" {
        return Err(Error::new(
            "CACHE_PROTOCOL",
            "checkpoint response schema differs",
        ));
    }
    if response["status"] == "NEEDS_INPUT" {
        if response.get("output").is_some()
            || response["certifiedResult"] != false
            || response["guardedExecutionAttempted"] != true
        {
            return Err(Error::new(
                "CACHE_PROTOCOL",
                "incomplete checkpoint carried a result or lacked its veto",
            ));
        }
        let discovery = json!({"schema":"svm-sysvar-discovery/v1","status":"NEEDS_INPUT","pubkey":response["pubkey"],"reads":response["reads"]});
        let discovery =
            _2_prepare_state::context::parse_sysvar_discovery_response(&discovery, &available)?;
        return Err(Error::new(
            "NEEDS_INPUT",
            "checkpoint attempt requires exact historical context",
        )
        .with_details(discovery));
    }
    if response["status"] == "UNSUPPORTED" {
        let mut failure = json!({"schema":"svm-epoch-stake-input-failure/v1"});
        for name in [
            "status",
            "code",
            "reason",
            "guardedExecutionAttempted",
            "certifiedResult",
            "targetSlot",
            "epoch",
            "read",
        ] {
            failure[name] = field(&response, name)?.clone();
        }
        _2_prepare_state::context::validate_epoch_stake_failure(&failure, fixture)?;
        return Err(Error::new(
            "UNSUPPORTED_HISTORICAL_EPOCH_STAKE",
            "exact current-epoch snapshot is unavailable",
        )
        .with_details(response));
    }
    let observed = checkpoint_response(&response, mode, prefix_count)?;
    _2_prepare_state::context::parse_sysvar_discovery_response(
        &json!({"schema":"svm-sysvar-discovery/v1","status":"COMPLETE","output":null,"reads":observed.reads}),
        &available,
    )?;
    if let Some(expected) = expected {
        if &observed.checkpoint_sha256 != expected
            || Digest::of(
                read_bounded_file(&checkpoint_path, limits.max_output_bytes)
                    .map_err(protocol_error)?,
            ) != *expected
        {
            return Err(Error::new(
                "CHECKPOINT_INTEGRITY",
                "restore mutated or misreported checkpoint bytes",
            ));
        }
    }
    totals.add(&observed.metrics)?;
    Ok(observed)
}
