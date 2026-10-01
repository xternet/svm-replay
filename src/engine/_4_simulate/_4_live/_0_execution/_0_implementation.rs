use super::*;

/// Prepare/hydrate through simulation first. A newly discovered input terminates
/// this session explicitly; user actions are never replayed automatically.
pub fn debug_prepared(
    mut request: PreparedRequest,
    config: &Config,
    cancel: CancellationToken,
    driver: &mut DebugDriver,
) -> Result<Value, Error> {
    let started = Instant::now();
    let options = config.trace.as_ref().ok_or_else(|| {
        Error::new(
            "DEBUG_CONFIG",
            "explicit interpreter-debug capture options required",
        )
    })?;
    if options.capture.execution_mode != "interpreter-debug" || options.capture.level == "off" {
        return Err(Error::new(
            "DEBUG_CONFIG",
            "live debugging requires interpreter-debug observations",
        ));
    }
    options.bounds.validate(&options.capture)?;
    _0_validate::run(&request)?;
    let budget = ExecutionBudget::new(Duration::from_millis(request.limits.timeout_ms), cancel)
        .map_err(_4_simulate::worker_error)?;
    let (job, mut receipt, coordinator) = _5_finalize::job::begin_job(
        &serde_json::to_value(&request)
            .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?,
        config,
        "supplied-prepared-debug/v1",
    )?;
    let result = (|| {
        let catalog =
            _1_resolve_runtime::load_catalog(&config.catalog_path, &config.catalog_sha256)?;
        let reference = _1_resolve_runtime::resolve(&catalog, &request)?;
        let capture = _1_resolve_runtime::resolve_capture(&catalog, &request)?;
        let complete = request.fixture["runtime"]["bankContext"]["requiredRuntimeSysvars"]
            .as_array()
            .ok_or_else(|| Error::new("INVALID_REQUEST", "runtime requirements missing"))?
            .iter()
            .any(|row| row["requirement"] == "complete-generic-sysvar-context-v1");
        if !complete {
            let inputs = _2_prepare_state::discovery::initial_inputs(&request.fixture)?;
            request.fixture =
                _2_prepare_state::discovery::tracked_fixture(&request.fixture, &inputs)?;
        }
        _2_prepare_state::validate_boundary(&request, &request.archived_block()?)?;
        let prepared_hash = _5_finalize::write_json(
            &job.join("prepared-request.json"),
            &serde_json::to_value(&request)
                .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?,
        )?;
        receipt["preparedRequest"] = json!({"file":"prepared-request.json","sha256":prepared_hash});
        receipt["worker"] = serde_json::to_value(&reference.descriptor)
            .map_err(|e| Error::new("OUTPUT_JSON", e.to_string()))?;
        receipt["cache"] = json!({"status":"BYPASSED_FOR_LIVE_DEBUG","newExecution":true});
        let transport = WorkerTransport::new(&job).with_owner(config.owner.clone());
        let limits = WorkerLimits {
            max_input_bytes: 256 * 1024 * 1024,
            max_output_bytes: request.limits.max_output_bytes as usize,
            max_diagnostic_bytes: request.limits.max_diagnostic_bytes as usize,
            cleanup_grace: Duration::from_secs(2),
        };
        let live = _4_simulate::debug::execute_pinned(
            &request,
            &reference,
            &capture,
            _4_simulate::trace::TraceContext {
                transport: &transport,
                scratch_root: &job,
                implementation_sha256: &coordinator.sha256,
            },
            &options.capture,
            &options.bounds,
            &limits,
            &budget,
            driver,
            &coordinator,
        )?;
        receipt["controlVerification"] = live.control_verification.clone();
        receipt["controlEvidence"] = live.control_evidence.clone();
        _1_resolve_runtime::load_catalog(&config.catalog_path, &config.catalog_sha256)?;
        budget.check().map_err(_4_simulate::worker_error)?;
        coordinator.verify()?;
        let mut exports = Vec::new();
        for (phase, export) in live.exports {
            if !["original-control", "requested"].contains(&phase.as_str()) {
                return Err(Error::new(
                    "DEBUG_PROTOCOL",
                    "unknown finalized export phase",
                ));
            }
            let file = format!("debug-{phase}.json");
            coordinator.verify()?;
            let sha = _5_finalize::write_bytes(&job.join(&file), &export.payload)?;
            exports
                .push(json!({"phase":phase,"file":file,"sha256":sha,"artifact":export.artifact}));
        }
        let events = _5_finalize::write_json(&job.join("debug-events.json"), &json!(live.events))?;
        receipt["debug"] = json!({"execution":live.receipt,"exports":exports,"events":{"file":"debug-events.json","sha256":events}});
        receipt["controlVerification"] = live.control_verification;
        receipt["verification"] = live.verification;
        Ok(live.output)
    })();
    let result = coordinator.check_result(result);
    if let Err(error) = &result {
        driver.failed(error)?;
    }
    _5_finalize::job::finish_job(result, receipt, &job, started)
}
