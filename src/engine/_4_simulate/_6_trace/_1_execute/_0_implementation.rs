use super::*;

pub fn execute(
    request: &PreparedRequest,
    reference: &ResolvedWorker,
    capture: &ResolvedCaptureWorker,
    context: TraceContext<'_>,
    policy: &CaptureRequest,
    bounds: &ProducerBounds,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
) -> Result<TraceExecution, Error> {
    let started = Instant::now();
    let budget = budget
        .narrowed(Duration::from_millis(policy.limits.timeout_ms))
        .map_err(protocol_error)?;
    request.validate()?;
    validate_worker_binding(request, &reference.descriptor)?;
    check_adapter(capture, reference, policy, bounds)?;
    verify_pin(reference, &budget)?;
    verify_pin(&capture.worker, &budget)?;
    let block = request.archived_block()?;
    _2_prepare_state::validate_boundary(request, &block)?;
    let canonical = _2_prepare_state::original_control(&request.fixture)?;
    let has_variant = _2_prepare_state::has_variant(&request.fixture)?;
    // The raw worker envelope includes execution results and encoded journals.
    // Its transport ceiling is separate from the requested exported-trace budget.
    let work = tempfile::Builder::new()
        .prefix("trace-attempt-")
        .tempdir_in(context.scratch_root)
        .map_err(|error| Error::new("TRACE_IO", error.to_string()))?;
    let mut events = Vec::new();
    let mut exports = Vec::new();
    let mut baseline_responses = Vec::new();
    let mut control_evidence = None;
    let result: Result<(Value, Value, Value, Value), Error> = (|| {
        let options = json!({"register_limit":bounds.register_rows,"memory_limit":bounds.memory_rows,"program_ids":policy.filter.program_ids,
            "instruction_indices":policy.filter.instruction_indices,"debug_port":null,"journal":if policy.level=="off"{Value::Null}else{
                json!({"max_events":policy.limits.max_events,"max_bytes":policy.limits.max_bytes,"program_ids":policy.filter.program_ids,"instruction_indices":policy.filter.instruction_indices})}});
        let options_path = work.path().join("options.json");
        let options_bytes = canonical_json(&options);
        let mut open = OpenOptions::new();
        open.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            open.mode(0o400);
        }
        let mut file = open
            .open(&options_path)
            .map_err(|error| Error::new("TRACE_IO", error.to_string()))?;
        file.write_all(options_bytes.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| Error::new("TRACE_IO", error.to_string()))?;
        let check_options = || -> Result<(), Error> {
            let bytes = read_bounded_file(&options_path, options_bytes.len())
                .map_err(|error| Error::new("TRACE_OPTIONS_INTEGRITY", error.to_string()))?;
            if bytes != options_bytes.as_bytes() {
                return Err(problem(
                    "TRACE_OPTIONS_INTEGRITY",
                    "capture controls changed during execution",
                ));
            }
            Ok(())
        };
        let mut phase = |fixture: &Value, name: &str| -> Result<(Value, Value), Error> {
            let tracked = fixture["runtime"]["bankContext"]["requiredRuntimeSysvars"]
                .as_array()
                .ok_or_else(|| problem("TRACE_PROTOCOL", "runtime requirements missing"))?
                .iter()
                .any(|entry| entry["requirement"] == "tracked-generic-sysvar-context-v1");
            _2_prepare_state::context::available_generic_sysvars(fixture)?;
            let mut run = |worker: &ResolvedWorker,
                           flags: &[String],
                           mode: &str|
             -> Result<Value, Error> {
                check_options()?;
                let execution = context
                    .transport
                    .run(&worker.spec, fixture, flags, limits, &budget)
                    .map_err(protocol_error)?;
                let bytes = canonical_json(&execution.value);
                events.push(json!({"phase":name,"mode":mode,"responseSha256":Digest::of(&bytes),"responseBytes":bytes.len(),
                    "process":{"pid":execution.pid,"elapsedMs":execution.elapsed.as_millis(),"cleanup":format!("{:?}",execution.cleanup_scope),
                        "stdoutBase64":STANDARD.encode(execution.stdout),"stderrBase64":STANDARD.encode(execution.stderr)}}));
                check_options()?;
                Ok(execution.value)
            };
            let reference_flags = if tracked {
                vec!["--discover-sysvars".into()]
            } else {
                Vec::new()
            };
            let original = run(reference, &reference_flags, "reference")?;
            let output = payload(&original, fixture, tracked)?;
            let verification = verify(request, fixture, &block, &output)?;
            if name == "original-control" {
                control_evidence = Some(super::super::super::control::evidence(
                    fixture,
                    &output,
                    &verification,
                ));
            }
            let off = run(&capture.worker, &["off".into()], "off")?;
            payload(&off, fixture, tracked)?;
            exact(&original, &off, "reference/off")?;
            baseline_responses.push((name.to_owned(), original));
            if policy.level != "off" {
                let captured = run(
                    &capture.worker,
                    &["--capture".into(), options_path.display().to_string()],
                    "capture",
                )?;
                if captured["schema"] != "svm-historical-capture-worker-experimental/v1" {
                    return Err(problem(
                        "TRACE_PROTOCOL",
                        "capture schema/execution mode differs",
                    ));
                }
                let observed = captured
                    .get("output")
                    .ok_or_else(|| problem("TRACE_PROTOCOL", "capture output missing"))?;
                payload(observed, fixture, tracked)?;
                if captured["status"] != "COMPLETE" {
                    return Err(problem(
                        "TRACE_PROTOCOL",
                        "capture execution did not finalize",
                    ));
                }
                crate::shared::trace::validate_execution_mode(&captured, &policy.execution_mode)?;
                exact(&off, observed, "off/capture")?;
                let captures = captured["captures"]
                    .as_array()
                    .ok_or_else(|| problem("TRACE_PROTOCOL", "capture list missing"))?;
                for execution in captures {
                    let rows = execution["trace"]
                        .as_array()
                        .ok_or_else(|| problem("TRACE_PROTOCOL", "register rows missing"))?;
                    let invocations = execution["invocations"]
                        .as_array()
                        .ok_or_else(|| problem("TRACE_PROTOCOL", "invocation metadata missing"))?;
                    if execution["execution_mode"] != policy.execution_mode
                        || rows.len() as u64 > bounds.register_rows
                        || invocations.len() as u64 > bounds.max_invocations
                    {
                        return Err(problem(
                            "CAPTURE_LIMIT",
                            "producer register/invocation/mode bound differs",
                        ));
                    }
                    match bounds.memory_rows {
                        None if execution["memory"] != Value::Null => {
                            return Err(problem("TRACE_PROTOCOL", "unrequested memory capture"))
                        }
                        Some(maximum)
                            if !execution["memory"]["rows"]
                                .as_array()
                                .is_some_and(|rows| rows.len() as u64 <= maximum) =>
                        {
                            return Err(problem("CAPTURE_LIMIT", "memory producer bound differs"))
                        }
                        _ => {}
                    }
                }
                let capture_key = capture_identity(
                    &Digest::of(canonical_json(fixture)),
                    &capture.worker.descriptor,
                    policy,
                    bounds,
                )?;
                let identity = Digest::of(canonical_json(
                    &json!({"schema":"svm-m17-gated-capture/v1","captureIdentity":capture_key,
                    "gateSha256":capture.gate_sha256,"referenceSha256":capture.reference_sha256,"implementationSha256":context.implementation_sha256,
                    "phase":name,"blockSha256":request.block_sha256,"candidate":request.candidate,"sourceEvidenceHashes":request.source_evidence_hashes}),
                ));
                exports.push((name.to_owned(), make_export(policy, &identity, &captured)?));
            }
            budget.check().map_err(protocol_error)?;
            Ok((output, verification))
        };
        let (control, control_verification) = phase(&canonical, "original-control")?;
        let (output, verification) = if has_variant {
            phase(&request.fixture, "requested")?
        } else {
            (control, control_verification.clone())
        };
        verify_pin(reference, &budget)?;
        verify_pin(&capture.worker, &budget)?;
        let artifacts: Vec<_> = exports
            .iter()
            .map(|(phase, export)| json!({"phase":phase,"artifact":export.artifact}))
            .collect();
        let receipt = json!({"schema":"svm-m17-trace-execution/v1","status":"PASS","executionMode":policy.execution_mode,"level":policy.level,
            "historicalVerificationSeparate":true,"currentStateFallback":false,"newExecution":true,"referenceSha256":capture.reference_sha256,
            "captureWorkerSha256":capture.worker.descriptor.sha256,"gateSha256":capture.gate_sha256,"implementationSha256":context.implementation_sha256,
            "workerCalls":events.len(),"elapsedMs":started.elapsed().as_millis(),"events":events,"exports":artifacts,"controlEvidence":control_evidence});
        Ok((output, verification, control_verification, receipt))
    })();
    if let Err(error) = work.close() {
        return Err(super::super::super::control::attach(
            Error::new("TRACE_CLEANUP_ERROR", error.to_string())
                .with_details(json!({"primaryError":result.err(),"events":events})),
            control_evidence.as_ref(),
        ));
    }
    result
        .map(
            |(output, verification, control_verification, receipt)| TraceExecution {
                output,
                verification,
                control_verification,
                exports,
                receipt,
                baseline_responses,
            },
        )
        .map_err(|mut error| {
            let cause = error.details.take();
            super::super::super::control::attach(
                error.with_details(
                    json!({"cause":cause,"events":events,"partialExportPublished":false}),
                ),
                control_evidence.as_ref(),
            )
        })
}
