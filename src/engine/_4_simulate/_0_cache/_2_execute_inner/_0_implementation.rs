use super::*;

pub(in super::super) fn execute_inner(
    request: &PreparedRequest,
    worker: &ResolvedWorker,
    context: CacheContext<'_>,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    options: CacheOptions,
    coordinator: Option<&CoordinatorPin>,
) -> Result<Value, Error> {
    let started = Instant::now();
    admit(request, worker, limits, budget)?;
    let block = request.archived_block()?;
    _2_prepare_state::validate_boundary(request, &block)?;
    let canonical = _2_prepare_state::original_control(&request.fixture)?;
    let has_variant = _2_prepare_state::has_variant(&request.fixture)?;
    let prefix_count = field(field(&canonical, "target")?, "prefixIndices")?
        .as_array()
        .ok_or_else(|| Error::new("CACHE_IDENTITY", "prefix indices must be array"))?
        .len();
    let keys = identity::derive(
        request,
        &worker.descriptor,
        context.catalog_sha256,
        context.implementation_sha256,
    )?;
    verify_worker(worker, budget)?;
    let work = tempfile::Builder::new()
        .prefix("cache-attempt-")
        .tempdir_in(context.scratch_root)
        .map_err(|error| Error::new("CACHE_IO", error.to_string()))?;
    let owner = work
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::new("CACHE_IO", "non-UTF8 attempt owner"))?
        .to_owned();
    let lease = attempt_lease(&owner)?;
    let checkpoint_path = work.path().join("checkpoint.json");
    let mut held = Vec::new();
    let mut events = Vec::new();
    let mut totals = Metrics::default();
    let mut control_evidence = None;
    let result = (|| {
        if options.result_cache {
            if let Some(bytes) = get(context.store, "result", &keys.result_key, &lease, &mut held)?
            {
                let record = parse_json(&bytes)?;
                let checkpoint_bytes =
                    admit_cached_result(&record, &keys, context.implementation_sha256, limits)?;
                let control_output = field(&record, "controlOutput")?;
                let output = field(&record, "output")?;
                for value in [control_output, output] {
                    if canonical_json(value).len() > limits.max_output_bytes {
                        return Err(Error::new(
                            "WORKER_OUTPUTLIMIT",
                            "cached output exceeds current byte limit",
                        ));
                    }
                }
                let control_verification = verify(request, &canonical, &block, control_output)?;
                control_evidence = Some(super::super::super::control::evidence(
                    &canonical,
                    control_output,
                    &control_verification,
                ));
                let verification = if has_variant {
                    verify(request, &request.fixture, &block, output)?
                } else {
                    if output != control_output {
                        return Err(Error::new(
                            "RESULT_EVIDENCE",
                            "unmodified cached output differs from control",
                        ));
                    }
                    control_verification.clone()
                };
                if field(&record, "controlVerification")? != &control_verification
                    || field(&record, "verification")? != &verification
                {
                    return Err(Error::new(
                        "RESULT_EVIDENCE",
                        "cached verification differs from recomputed evidence",
                    ));
                }
                verify_worker(worker, budget)?;
                publish(coordinator, || Ok(()))?;
                return Ok(
                    json!({"schema":"svm-m17-cache-execution/v1","status":"PASS","resultCacheHit":true,"preparedCacheHit":null,
                    "preparedKey":keys.prepared_key,"resultKey":keys.result_key,"checkpointSha256":record["checkpointSha256"],"checkpointBytes":checkpoint_bytes,
                    "output":output,"verification":verification,"controlVerification":control_verification,"controlEvidence":control_evidence,"currentStateFallback":false,"newExecution":false,
                    "events":events,"metrics":{"prefixTransactionsExecuted":0,"prefixSimulationCalls":0,"prefixCommitCalls":0,"prefixTransactionsReused":0,"workerCalls":0,"elapsedMs":started.elapsed().as_millis()}}),
                );
            }
        }
        let cached = get(
            context.store,
            "prepared",
            &keys.prepared_key,
            &lease,
            &mut held,
        )?;
        let prepared_hit = cached.is_some();
        let mut run = |fixture: &Value,
                       mode: CheckpointMode,
                       phase: &str,
                       expected: Option<&Digest>|
         -> Result<boundary::CheckpointResponse, Error> {
            checkpoint_run(
                fixture,
                mode,
                phase,
                expected,
                &context,
                worker,
                limits,
                budget,
                &checkpoint_path,
                prefix_count,
                &mut events,
                &mut totals,
            )
        };
        let checkpoint = match cached {
            Some(bytes) => {
                if bytes.len() > limits.max_output_bytes {
                    return Err(Error::new(
                        "WORKER_OUTPUTLIMIT",
                        "cached checkpoint exceeds current file limit",
                    ));
                }
                validate_checkpoint(&bytes, &canonical, &worker.descriptor.sha256)?;
                write_checkpoint(&checkpoint_path, &bytes)?;
                bytes
            }
            None => {
                let observed = run(&canonical, CheckpointMode::Create, "prefix", None)?;
                let bytes = read_bounded_file(&checkpoint_path, limits.max_output_bytes)
                    .map_err(protocol_error)?;
                if validate_checkpoint(&bytes, &canonical, &worker.descriptor.sha256)?
                    != observed.checkpoint_sha256
                {
                    return Err(Error::new(
                        "CHECKPOINT_INTEGRITY",
                        "created checkpoint differs from worker receipt",
                    ));
                }
                bytes
            }
        };
        let checkpoint_sha256 = Digest::of(&checkpoint);
        let control = run(
            &canonical,
            CheckpointMode::Run,
            "original-control",
            Some(&checkpoint_sha256),
        )?
        .output
        .ok_or_else(|| Error::new("CACHE_PROTOCOL", "control output missing"))?;
        let control_verification = verify(request, &canonical, &block, &control)?;
        control_evidence = Some(super::super::super::control::evidence(
            &canonical,
            &control,
            &control_verification,
        ));
        let (output, verification) = if has_variant {
            let output = run(
                &request.fixture,
                CheckpointMode::Run,
                "requested",
                Some(&checkpoint_sha256),
            )?
            .output
            .ok_or_else(|| Error::new("CACHE_PROTOCOL", "requested output missing"))?;
            let verified = verify(request, &request.fixture, &block, &output)?;
            (output, verified)
        } else {
            (control.clone(), control_verification.clone())
        };
        verify_worker(worker, budget)?;
        if !prepared_hit {
            publish(coordinator, || {
                put(
                    context.store,
                    "prepared",
                    &keys.prepared_key,
                    &checkpoint,
                    &lease,
                    &mut held,
                )
            })?;
        }
        let stored = json!({"schema":"svm-m17-cached-result/v1","preparedKey":keys.prepared_key,"resultKey":keys.result_key,
            "implementationSha256":context.implementation_sha256,"checkpointSha256":checkpoint_sha256,"checkpointBytes":checkpoint.len(),
            "controlOutput":control,"output":output,"verification":verification,"controlVerification":control_verification});
        if options.result_cache {
            publish(coordinator, || {
                put(
                    context.store,
                    "result",
                    &keys.result_key,
                    canonical_json(&stored).as_bytes(),
                    &lease,
                    &mut held,
                )
            })?;
        }
        budget.check().map_err(protocol_error)?;
        let mut metrics = serde_json::to_value(&totals)
            .map_err(|error| Error::new("CACHE_PROTOCOL", error.to_string()))?;
        metrics["workerCalls"] = json!(events.len());
        metrics["elapsedMs"] = json!(started.elapsed().as_millis());
        Ok(
            json!({"schema":"svm-m17-cache-execution/v1","status":"PASS","resultCacheHit":false,"preparedCacheHit":prepared_hit,
            "preparedKey":keys.prepared_key,"resultKey":keys.result_key,"checkpointSha256":checkpoint_sha256,"checkpointBytes":checkpoint.len(),
            "output":output,"verification":verification,"controlVerification":control_verification,"controlEvidence":control_evidence,"currentStateFallback":false,"newExecution":true,"events":events,"metrics":metrics}),
        )
    })();
    let mut cleanup_errors = Vec::new();
    for namespace in held {
        let key = if namespace == "prepared" {
            &keys.prepared_key
        } else {
            &keys.result_key
        };
        match context.store.execute(json!({"version":1,"op":"release","namespace":namespace,"key":key,"owner":owner})) {
            Ok(response) if response["status"] == "OK" => {}
            Ok(response) => cleanup_errors.push(json!({"namespace":namespace,"error":"unknown release response","response":response})),
            Err(error) => cleanup_errors.push(json!({"namespace":namespace,"error":store_error(error)})),
        }
    }
    if let Err(error) = work.close() {
        cleanup_errors.push(json!({"scratch":error.to_string()}));
    }
    if !cleanup_errors.is_empty() {
        return Err(super::super::super::control::attach(Error::new("CACHE_CLEANUP_ERROR", "cache ownership/scratch release failed")
            .with_details(json!({"errors":cleanup_errors,"primaryError":result.err(),"preparedKey":keys.prepared_key,"resultKey":keys.result_key})), control_evidence.as_ref()));
    }
    result.map_err(|mut error| { let cause = error.details.take(); super::super::super::control::attach(error.with_details(json!({"cause":cause,"preparedKey":keys.prepared_key,"resultKey":keys.result_key,"events":events})), control_evidence.as_ref()) })
}
