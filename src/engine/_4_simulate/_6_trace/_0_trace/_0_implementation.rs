use super::*;

pub struct TraceContext<'a> {
    pub transport: &'a WorkerTransport,
    pub scratch_root: &'a Path,
    pub implementation_sha256: &'a Digest,
}

pub struct TraceExecution {
    pub output: Value,
    pub verification: Value,
    pub control_verification: Value,
    pub exports: Vec<(String, CaptureExport)>,
    pub receipt: Value,
    /// In-memory exact guarded responses for a subsequent same-context mode gate.
    /// These are intentionally not duplicated into persisted receipt metadata.
    pub baseline_responses: Vec<(String, Value)>,
}

pub(in super::super) fn problem(code: &str, message: &str) -> Error {
    Error::new(code, message)
}

pub(in super::super::super) fn native_execution_mode() -> &'static str {
    if cfg!(all(target_arch = "x86_64", not(windows))) {
        "jit"
    } else {
        "interpreter"
    }
}

pub(in super::super) fn verify_pin(
    worker: &ResolvedWorker,
    budget: &ExecutionBudget,
) -> Result<(), Error> {
    budget.check().map_err(protocol_error)?;
    if worker.spec.sha256 != worker.descriptor.sha256.as_str()
        || file_sha256(&worker.spec.executable).map_err(protocol_error)? != worker.spec.sha256
    {
        return Err(problem(
            "WORKER_IDENTITY",
            "trace worker bytes/descriptor differ",
        ));
    }
    budget.check().map_err(protocol_error)
}

pub(in super::super) fn check_adapter(
    capture: &ResolvedCaptureWorker,
    reference: &ResolvedWorker,
    policy: &CaptureRequest,
    bounds: &ProducerBounds,
) -> Result<(), Error> {
    let worker = &capture.worker.descriptor;
    if capture.reference_sha256 != reference.descriptor.sha256
        || worker.family != reference.descriptor.family
        || worker.executor_source_id != reference.descriptor.executor_source_id
    {
        return Err(problem(
            "WORKER_IDENTITY",
            "capture gate/reference family/source differs",
        ));
    }
    validate_capability(policy, worker, worker)?;
    bounds.validate(policy)?;
    let newest =
        worker.family == "v4-2" && worker.executor_source_id == "litesvm-v0.16.0-agave-4.2.1";
    if !newest
        && !worker
            .capabilities
            .iter()
            .any(|value| value == "runtime-full-capture-cli/v1")
    {
        return Err(problem(
            "CAPABILITY_UNAVAILABLE",
            "worker lacks exact full-capture CLI adapter",
        ));
    }
    // Memory support is established by the pinned worker capability above,
    // not by its runtime age. Older, unqualified bundles still reject it.
    if policy.execution_mode != native_execution_mode() {
        return Err(problem(
            "CAPABILITY_UNAVAILABLE",
            "capture mode differs from native execution; interpreter-debug requires a live debug session",
        ));
    }
    Ok(())
}

pub(in super::super::super) fn payload(
    value: &Value,
    fixture: &Value,
    tracked: bool,
) -> Result<Value, Error> {
    let available = _2_prepare_state::context::available_generic_sysvars(fixture)?;
    if value["schema"] == "svm-epoch-stake-input-failure/v1" {
        _2_prepare_state::context::validate_epoch_stake_failure(value, fixture)?;
        return Err(problem(
            "UNSUPPORTED_HISTORICAL_EPOCH_STAKE",
            "exact epoch snapshot is unavailable",
        )
        .with_details(value.clone()));
    }
    if value["schema"] == "svm-sysvar-discovery/v1" {
        let parsed = _2_prepare_state::context::parse_sysvar_discovery_response(value, &available)?;
        if parsed["status"] == "NEEDS_INPUT" {
            return Err(problem(
                "NEEDS_INPUT",
                "trace attempt requires exact historical context",
            )
            .with_details(parsed));
        }
        return parsed
            .get("output")
            .cloned()
            .ok_or_else(|| problem("TRACE_PROTOCOL", "guarded output missing"));
    }
    if tracked || value["schema"] != "svm-simulate-m6-executor-output/v1" {
        return Err(problem(
            "TRACE_PROTOCOL",
            "trace execution lacked required guarded output",
        ));
    }
    Ok(value.clone())
}

pub(in super::super::super) fn verify(
    request: &PreparedRequest,
    fixture: &Value,
    block: &Value,
    output: &Value,
) -> Result<Value, Error> {
    let policy = match request.metadata_policy {
        MetadataPolicy::Strict => _3_verify_original::MetadataPolicy::Strict,
        MetadataPolicy::ArchivedComputeMeterWarning => {
            _3_verify_original::MetadataPolicy::ArchivedComputeMeterWarning
        }
    };
    _3_verify_original::verify_with_options(
        &request.candidate,
        fixture,
        block,
        output,
        policy,
        false,
    )
    .map(|verified| verified.0)
    .map_err(|error| Error::new("MISMATCH", error.to_string()))
}

pub(in super::super) fn exact(left: &Value, right: &Value, phase: &str) -> Result<(), Error> {
    if canonical_json(left) != canonical_json(right) {
        return Err(Error::new("TRACE_PARITY_MISMATCH",format!("{phase}: capture changed exact canonical execution"))
            .with_details(json!({"referenceSha256":Digest::of(canonical_json(left)),"observedSha256":Digest::of(canonical_json(right))})));
    }
    Ok(())
}
