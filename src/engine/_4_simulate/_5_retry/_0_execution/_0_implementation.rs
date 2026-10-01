use super::*;

pub(crate) fn execute(
    request: &PreparedRequest,
    config: &Config,
    job: &std::path::Path,
    budget: &ExecutionBudget,
    receipt: &mut Value,
    coordinator: &CoordinatorPin,
) -> Result<Value, Error> {
    execute_hydrated(
        request,
        config,
        job,
        budget,
        receipt,
        coordinator,
        |key, slot| {
            Err(Error::new("NEEDS_INPUT",format!("exact {key} at slot {slot} not supplied; no provider is configured for a prepared-only request")))
        },
    )
}

pub(crate) fn execute_hydrated(
    request: &PreparedRequest,
    config: &Config,
    job: &std::path::Path,
    budget: &ExecutionBudget,
    receipt: &mut Value,
    coordinator: &CoordinatorPin,
    mut hydrate: impl FnMut(&str, u64) -> Result<(Value, Vec<Digest>), Error>,
) -> Result<Value, Error> {
    let mut ordinal = 0;
    let evidence = RefCell::new(
        request
            .source_evidence_hashes
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
    );
    let settled = _2_prepare_state::discovery::settle(
        &request.fixture,
        |fixture| {
            budget.check().map_err(worker_error)?;
            receipt["phases"] = json!([]);
            receipt["controlVerification"] = Value::Null;
            receipt["verification"] = Value::Null;
            if let Some(object) = receipt.as_object_mut() {
                object.remove("cache");
                object.remove("trace");
                object.remove("controlEvidence");
            }
            let mut prepared = request.clone();
            prepared.fixture = fixture.clone();
            prepared.source_evidence_hashes = evidence.borrow().iter().cloned().collect();
            let mut result = coordinator.check_result(_4_simulate::dispatch::execute(
                &prepared,
                config,
                job,
                budget,
                receipt,
                coordinator,
            ));
            if let Err(error) = &mut result {
                archive_failed_attempt(job, receipt, ordinal, error)?;
            }
            ordinal += 1;
            result
        },
        |key, slot| {
            let (account, hashes) = hydrate(key, slot)?;
            evidence.borrow_mut().extend(hashes);
            Ok(account)
        },
    )?;
    coordinator.verify()?;
    let fixture_hash = _5_finalize::write_json(&job.join("prepared.json"), &settled.fixture)?;
    let evidence = evidence.into_inner();
    let mut reusable = request.clone();
    reusable.fixture = settled.fixture;
    reusable.source_evidence_hashes = evidence.iter().cloned().collect();
    let reusable_hash = _5_finalize::write_json(
        &job.join("prepared-request.json"),
        &serde_json::to_value(&reusable).map_err(|e| Error::new("OUTPUT_JSON", e.to_string()))?,
    )?;
    receipt["preparedRequest"] = json!({"file":"prepared-request.json","sha256":reusable_hash});
    receipt["preparation"] = json!({"fixture":{"file":"prepared.json","sha256":fixture_hash},
        "discoveryAttempts":settled.attempts,"attemptCount":ordinal,"sourceEvidenceHashes":evidence});
    Ok(settled.output)
}
