use super::*;

pub(in super::super) fn execute_inner(
    request: &PreparedRequest,
    reference: &ResolvedWorker,
    capture: &ResolvedCaptureWorker,
    context: TraceContext<'_>,
    policy: &CaptureRequest,
    bounds: &ProducerBounds,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    driver: &mut DebugDriver,
    coordinator: Option<&CoordinatorPin>,
) -> Result<DebugExecution, Error> {
    let started = Instant::now();
    let (jit_policy, jit_receipt, mut control_evidence, baseline) = native_gate(
        request, reference, capture, &context, policy, bounds, limits, budget,
    )?;
    let original = _2_prepare_state::original_control(&request.fixture)
        .map_err(|error| super::super::super::control::attach(error, control_evidence.as_ref()))?;
    let block = request
        .archived_block()
        .map_err(|error| super::super::super::control::attach(error, control_evidence.as_ref()))?;
    let work = tempfile::Builder::new()
        .prefix("debug-attempt-")
        .tempdir_in(context.scratch_root)
        .map_err(|e| {
            super::super::super::control::attach(
                Error::new("DEBUG_IO", e.to_string()),
                control_evidence.as_ref(),
            )
        })?;
    let mut events = Vec::new();
    let mut exports = Vec::new();
    let mut processes = Vec::new();
    let mut inventories = Vec::new();
    let mut effective_limits = limits.clone();
    effective_limits.max_output_bytes = effective_limits
        .max_output_bytes
        .min(policy.limits.max_bytes as usize);
    let result = (|| -> Result<(Value, Value, Value, Value), Error> {
        let mut phase = |name: &str, fixture: &Value| -> Result<(Value, Value), Error> {
            let reserved = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
                .map_err(|e| Error::new("DEBUG_ENDPOINT", e.to_string()))?;
            let port = reserved
                .local_addr()
                .map_err(|e| Error::new("DEBUG_ENDPOINT", e.to_string()))?
                .port();
            // Existing native workers bind their own ephemeral listener. Exact
            // notices + runtime identity are checked, but this is not host-authenticated IPC.
            drop(reserved);
            let options = json!({"register_limit":bounds.register_rows,"memory_limit":bounds.memory_rows,"program_ids":policy.filter.program_ids,"instruction_indices":policy.filter.instruction_indices,"debug_port":port,
                "journal":{"max_events":policy.limits.max_events,"max_bytes":policy.limits.max_bytes,"program_ids":policy.filter.program_ids,"instruction_indices":policy.filter.instruction_indices}});
            let option_bytes = canonical_json(&options);
            let option_path = work.path().join(format!("{name}-options.json"));
            let mut open = OpenOptions::new();
            open.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                open.mode(0o400);
            }
            let mut file = open
                .open(&option_path)
                .map_err(|e| Error::new("DEBUG_IO", e.to_string()))?;
            file.write_all(option_bytes.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|e| Error::new("DEBUG_IO", e.to_string()))?;
            let flags = vec!["--capture".into(), option_path.display().to_string()];
            let session_budget = budget
                .narrowed(Duration::from_millis(policy.limits.timeout_ms))
                .map_err(protocol_error)?;
            let live = run_session(
                context.transport,
                &capture.worker.spec,
                fixture,
                &flags,
                &effective_limits,
                &session_budget,
                port,
                bounds.max_invocations as usize,
                name,
                driver,
            )?;
            budget.check().map_err(protocol_error)?;
            require(
                read_bounded_file(&option_path, option_bytes.len()).map_err(protocol_error)?
                    == option_bytes.as_bytes(),
                "DEBUG_OPTIONS_INTEGRITY",
                "live debugger controls changed during execution",
            )?;
            let captured = &live.worker.value;
            let bytes = canonical_json(captured);
            processes.push(json!({"phase":name,"responseSha256":Digest::of(&bytes),"responseBytes":bytes.len(),"pid":live.worker.pid,"elapsedMs":live.worker.elapsed.as_millis(),"cleanup":format!("{:?}",live.worker.cleanup_scope),"stdoutBase64":STANDARD.encode(&live.worker.stdout),"stderrBase64":STANDARD.encode(&live.worker.stderr)}));
            events.extend(live.events);
            require(
                captured["schema"] == "svm-historical-capture-worker-experimental/v1",
                "DEBUG_PROTOCOL",
                "worker capture schema or execution mode differs",
            )?;
            let observed = captured
                .get("output")
                .ok_or_else(|| Error::new("DEBUG_PROTOCOL", "final worker output missing"))?;
            let tracked = fixture["runtime"]["bankContext"]["requiredRuntimeSysvars"]
                .as_array()
                .ok_or_else(|| Error::new("DEBUG_PROTOCOL", "runtime requirements missing"))?
                .iter()
                .any(|row| row["requirement"] == "tracked-generic-sysvar-context-v1");
            let output = trace::payload(observed, fixture, tracked)?;
            require(
                captured["status"] == "COMPLETE",
                "DEBUG_PROTOCOL",
                "whole-worker execution did not finalize",
            )?;
            crate::shared::trace::validate_execution_mode(captured, "interpreter-debug")?;
            let expected = &baseline
                .iter()
                .find(|(phase, _)| phase == name)
                .ok_or_else(|| Error::new("DEBUG_PROTOCOL", "same-context JIT baseline missing"))?
                .1;
            if canonical_json(expected) != canonical_json(observed) {
                return Err(Error::new("DEBUG_PARITY_MISMATCH","native/live-interpreter exact canonical output differs").with_details(json!({"phase":name,"baselineMode":jit_policy.execution_mode,"baselineSha256":Digest::of(canonical_json(expected)),"interpreterSha256":Digest::of(canonical_json(observed))})));
            }
            let verification = trace::verify(request, fixture, &block, &output)?;
            capture_bounds(captured, bounds)?;
            let inventory_policy = if capture.worker.descriptor.family == "v4-2"
                && capture.worker.descriptor.executor_source_id == "litesvm-v0.16.0-agave-4.2.1"
            {
                FinalizedInventoryPolicy::V42OrdinalAndCapturedIntersection
            } else {
                FinalizedInventoryPolicy::Complete
            };
            let inventory = validate_finalized_inventory(
                captured,
                &live.invocations,
                bounds.max_invocations as usize,
                inventory_policy,
            )?;
            let capture_key = capture_identity(
                &Digest::of(canonical_json(fixture)),
                &capture.worker.descriptor,
                policy,
                bounds,
            )?;
            let identity = Digest::of(canonical_json(
                &json!({"schema":"svm-m17-debug-identity/v1","captureIdentity":capture_key,"gateSha256":capture.gate_sha256,"referenceSha256":capture.reference_sha256,"implementationSha256":context.implementation_sha256,"phase":name,"blockSha256":request.block_sha256,"candidate":request.candidate,"sourceEvidenceHashes":request.source_evidence_hashes}),
            ));
            exports.push((name.to_owned(), make_export(policy, &identity, captured)?));
            publish(coordinator, || {
                driver.phase_complete(name, &inventory.execution_indices)
            })?;
            inventories.push(json!({"phase":name,"inventory":inventory}));
            Ok((output, verification))
        };
        let (control, control_verification) = phase("original-control", &original)?;
        let mut completed_control =
            super::super::super::control::evidence(&original, &control, &control_verification);
        let live_kind = if jit_policy.execution_mode == "jit" {
            "fresh-live-original-with-jit-parity"
        } else {
            "fresh-live-original-with-interpreter-parity"
        };
        completed_control["provenance"] = json!({"kind":live_kind,"executionMode":"interpreter-debug","liveInterpreterParityEstablished":true});
        control_evidence = Some(completed_control);
        let (output, verification) = if _2_prepare_state::has_variant(&request.fixture)? {
            phase("requested", &request.fixture)?
        } else {
            (control, control_verification.clone())
        };
        budget.check().map_err(protocol_error)?;
        require(
            file_sha256(&reference.spec.executable).map_err(protocol_error)?
                == reference.spec.sha256
                && file_sha256(&capture.worker.spec.executable).map_err(protocol_error)?
                    == capture.worker.spec.sha256,
            "WORKER_IDENTITY",
            "worker changed across JIT/debug mode gate",
        )?;
        let artifacts: Vec<_> = exports
            .iter()
            .map(|(phase, export)| json!({"phase":phase,"artifact":export.artifact}))
            .collect();
        let symbols = driver.symbol_admissions();
        let receipt = json!({"schema":"svm-m17-debug-execution/v1","status":"PASS","executionMode":"interpreter-debug","jitInterpreterParity":"exact canonical whole-worker output","historicalVerificationSeparate":true,"sourceLevelStepping":if symbols.is_empty(){"unavailable: assembly-only"}else{"exact-ELF finite observations; per-invocation runtime capabilities required; not historical source-availability certification"},"symbolAdmissions":symbols,"finalizedInventory":inventories,"currentStateFallback":false,"newExecution":true,"referenceSha256":capture.reference_sha256,"captureWorkerSha256":capture.worker.descriptor.sha256,"gateSha256":capture.gate_sha256,"implementationSha256":context.implementation_sha256,"jitGate":jit_receipt,"debugWorkerCalls":processes.len(),"elapsedMs":started.elapsed().as_millis(),"processes":processes,"eventCount":events.len(),"eventsSha256":Digest::of(canonical_json(&json!(events))),"exports":artifacts});
        let mut receipt = receipt;
        if jit_policy.execution_mode != "jit" {
            let fields = receipt.as_object_mut().expect("execution receipt object");
            let gate = fields.remove("jitGate").expect("fresh gate field");
            fields.remove("jitInterpreterParity").expect("parity field");
            fields.insert("nativeGate".into(), gate);
            fields.insert(
                "nativeInterpreterParity".into(),
                json!("exact canonical whole-worker output"),
            );
            fields.insert(
                "baselineExecutionMode".into(),
                json!(jit_policy.execution_mode),
            );
        }
        receipt["deadlinePolicy"] = json!({"captureTimeoutMs":policy.limits.timeout_ms,"captureScope":"per-fresh-JIT-gate-and-per-live-worker-session","jobScope":"unchanged caller-supplied cumulative deadline across gate, sessions, export and finalization","cancellation":"shared job token; never renewed"});
        if jit_policy.execution_mode != "jit" {
            receipt["deadlinePolicy"]["captureScope"] =
                json!("per-fresh-native-gate-and-per-live-worker-session");
        }
        Ok((output, verification, control_verification, receipt))
    })();
    if let Err(cleanup) = work.close() {
        return Err(super::super::super::control::attach(
            Error::new("DEBUG_CLEANUP", cleanup.to_string())
                .with_details(json!({"cause":result.err(),"events":events,"processes":processes})),
            control_evidence.as_ref(),
        ));
    }
    match result {
        Ok((output, verification, control_verification, receipt)) => {
            publish(coordinator, || driver.complete(&receipt)).map_err(|error| {
                super::super::super::control::attach(error, control_evidence.as_ref())
            })?;
            let control_evidence = control_evidence.ok_or_else(|| {
                Error::new(
                    "DEBUG_PROTOCOL",
                    "verified original-control evidence missing",
                )
            })?;
            Ok(DebugExecution {
                output,
                verification,
                control_verification,
                exports,
                events,
                receipt,
                control_evidence,
            })
        }
        Err(mut error) => {
            let cause = error.details.take();
            Err(super::super::super::control::attach(error.with_details(json!({"cause":cause,"events":events,"processes":processes,"partialExportPublished":false})), control_evidence.as_ref()))
        }
    }
}
