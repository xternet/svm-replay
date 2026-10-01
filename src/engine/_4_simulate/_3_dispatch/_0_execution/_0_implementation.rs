use super::*;

pub(crate) fn execute(
    request: &PreparedRequest,
    config: &Config,
    job: &std::path::Path,
    budget: &ExecutionBudget,
    receipt: &mut Value,
    coordinator: &CoordinatorPin,
) -> Result<Value, Error> {
    budget.check().map_err(worker_error)?;
    coordinator.verify()?;
    let catalog = _1_resolve_runtime::load_catalog(&config.catalog_path, &config.catalog_sha256)?;
    let worker = _1_resolve_runtime::resolve(&catalog, request)?;
    receipt["worker"] = serde_json::to_value(&worker.descriptor)
        .map_err(|e| Error::new("OUTPUT_JSON", e.to_string()))?;
    let block = request.archived_block()?;
    _2_prepare_state::validate_boundary(request, &block)?;
    let original = _2_prepare_state::original_control(&request.fixture)?;
    let transport = WorkerTransport::new(job).with_owner(config.owner.clone());
    let limits = WorkerLimits {
        max_input_bytes: 256 * 1024 * 1024,
        max_output_bytes: request.limits.max_output_bytes as usize,
        max_diagnostic_bytes: request.limits.max_diagnostic_bytes as usize,
        cleanup_grace: Duration::from_secs(2),
    };
    if let Some(options) = &config.trace {
        let capture = _1_resolve_runtime::resolve_capture(&catalog, request)?;
        let traced = _4_simulate::trace::execute(
            request,
            &worker,
            &capture,
            _4_simulate::trace::TraceContext {
                transport: &transport,
                scratch_root: job,
                implementation_sha256: &coordinator.sha256,
            },
            &options.capture,
            &options.bounds,
            &limits,
            budget,
        )
        .inspect_err(|error| {
            super::super::super::control::preserve(receipt, error);
        })?;
        receipt["controlVerification"] = traced.control_verification.clone();
        receipt["controlEvidence"] = traced.receipt["controlEvidence"].clone();
        // Export only after both immutable control and requested execution passed.
        _1_resolve_runtime::load_catalog(&config.catalog_path, &config.catalog_sha256)?;
        budget.check().map_err(worker_error)?;
        coordinator.verify()?;
        let mut exports = Vec::new();
        for (phase, export) in traced.exports {
            if !["original-control", "requested"].contains(&phase.as_str()) {
                return Err(Error::new("TRACE_PROTOCOL", "unknown export phase"));
            }
            let file = format!("trace-{phase}.json");
            coordinator.verify()?;
            let hash = _5_finalize::write_bytes(&job.join(&file), &export.payload)?;
            exports
                .push(json!({"phase":phase,"file":file,"sha256":hash,"artifact":export.artifact}));
        }
        let completeness = options.check_completeness(&exports);
        receipt["trace"] = json!({"execution":traced.receipt,"exports":exports});
        receipt["controlVerification"] = traced.control_verification;
        receipt["verification"] = traced.verification;
        receipt["cache"] = json!({"status":"BYPASSED_FOR_FRESH_TRACE","newExecution":true});
        completeness?;
        return Ok(traced.output);
    }
    if config.cache != CacheMode::Off {
        let mut store =
            svm_replay_store::Store::open(&crate::shared::layout::cache_dir(&config.data_dir)?)
                .map_err(|e| Error::new(format!("STORE_{}", e.code), e.message))?;
        let mut cached = _4_simulate::cache::execute_pinned(
            request,
            &worker,
            _4_simulate::cache::CacheContext {
                store: &mut store,
                transport: &transport,
                scratch_root: job,
                catalog_sha256: &config.catalog_sha256,
                implementation_sha256: &coordinator.sha256,
            },
            &limits,
            budget,
            _4_simulate::cache::CacheOptions {
                result_cache: config.cache == CacheMode::All,
            },
            coordinator,
        )
        .inspect_err(|error| {
            super::super::super::control::preserve(receipt, error);
        })?;
        receipt["controlVerification"] = cached["controlVerification"].clone();
        receipt["controlEvidence"] = cached["controlEvidence"].clone();
        receipt["verification"] = cached["verification"].clone();
        let output = cached
            .as_object_mut()
            .ok_or_else(|| Error::new("CACHE_PROTOCOL", "cache result must be object"))?
            .remove("output")
            .ok_or_else(|| Error::new("CACHE_PROTOCOL", "cache output missing"))?;
        receipt["cache"] = cached;
        _1_resolve_runtime::load_catalog(&config.catalog_path, &config.catalog_sha256)?;
        budget.check().map_err(worker_error)?;
        return Ok(output);
    }
    let policy = match request.metadata_policy {
        MetadataPolicy::Strict => _3_verify_original::MetadataPolicy::Strict,
        MetadataPolicy::ArchivedComputeMeterWarning => {
            _3_verify_original::MetadataPolicy::ArchivedComputeMeterWarning
        }
    };
    let control = _4_simulate::run(&transport, &worker.spec, &original, &limits, budget)?;
    receipt["phases"]
        .as_array_mut()
        .ok_or_else(|| Error::new("INTERNAL", "phase container changed"))?
        .push(json!({"phase":"original-control","process":control.process,"reads":control.reads}));
    let checked = match _3_verify_original::verify(
        &request.candidate,
        &original,
        &block,
        &control.output,
        policy,
    ) {
        Ok(checked) => checked,
        Err(error) => {
            return Err(super::super::super::control::retain_failure(
                job,
                &control.output,
                error.to_string(),
            )?)
        }
    };
    receipt["controlVerification"] =
        serde_json::to_value(&checked).map_err(|e| Error::new("OUTPUT_JSON", e.to_string()))?;
    receipt["controlEvidence"] = super::super::super::control::evidence(
        &original,
        &control.output,
        &receipt["controlVerification"],
    );
    let output = if _2_prepare_state::has_variant(&request.fixture)? {
        let requested =
            _4_simulate::run(&transport, &worker.spec, &request.fixture, &limits, budget)?;
        receipt["phases"]
            .as_array_mut()
            .ok_or_else(|| Error::new("INTERNAL", "phase container changed"))?
            .push(json!({"phase":"requested","process":requested.process,"reads":requested.reads}));
        let checked = _3_verify_original::verify(
            &request.candidate,
            &request.fixture,
            &block,
            &requested.output,
            policy,
        )
        .map_err(|e| Error::new("MISMATCH", e.to_string()))?;
        receipt["verification"] =
            serde_json::to_value(checked).map_err(|e| Error::new("OUTPUT_JSON", e.to_string()))?;
        requested.output
    } else {
        receipt["verification"] = receipt["controlVerification"].clone();
        control.output
    };
    budget.check().map_err(worker_error)?;
    _1_resolve_runtime::load_catalog(&config.catalog_path, &config.catalog_sha256)?;
    budget.check().map_err(worker_error)?;
    Ok(output)
}
