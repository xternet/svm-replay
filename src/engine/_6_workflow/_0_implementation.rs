use super::*;

pub fn simulate_prepared(
    request: PreparedRequest,
    config: &Config,
    cancel: CancellationToken,
) -> Result<Value, Error> {
    let started = Instant::now();
    let budget = ExecutionBudget::new(Duration::from_millis(request.limits.timeout_ms), cancel)
        .map_err(worker_error)?;
    _0_validate::run(&request)?;
    let (job, mut receipt, coordinator) = begin_job(
        &serde_json::to_value(&request)
            .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?,
        config,
        "supplied-prepared-replay/v1",
    )?;
    let result = execute(&request, config, &job, &budget, &mut receipt, &coordinator);
    finish_job(coordinator.check_result(result), receipt, &job, started)
}

/// A supplied fixture remains supplied; sources only hydrate guarded exact inputs.
pub fn simulate_prepared_with_sources(
    request: PreparedRequest,
    config: &Config,
    mut sources: CompositeSource,
    genesis_hash: String,
    cancel: CancellationToken,
) -> Result<Value, Error> {
    let started = Instant::now();
    let budget = ExecutionBudget::new(Duration::from_millis(request.limits.timeout_ms), cancel)
        .map_err(worker_error)?;
    _0_validate::run(&request)?;
    crate::shared::history::address(&genesis_hash, 32)?;
    _2_prepare_state::context::assert_epoch_stake_binding(&request.fixture, Some(&genesis_hash))?;
    _2_prepare_state::context::assert_initialized_stake_binding(
        &request.fixture,
        Some(&genesis_hash),
    )?;
    sources
        .assert_genesis(&genesis_hash)
        .map_err(crate::shared::history::source_error)?;
    let slot = request.candidate["slot"]
        .as_u64()
        .ok_or_else(|| Error::new("INVALID_REQUEST", "historical slot required"))?;
    let (job, mut receipt, coordinator) = begin_job(
        &serde_json::to_value(&request)
            .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?,
        config,
        "supplied-prepared-replay/v1",
    )?;
    receipt["sourceHydrationEnabled"] = json!(true);
    receipt["sourceGenesisHash"] = json!(genesis_hash);
    sources = sources
        .with_discovery_artifacts(&job.join("source-blocks"))
        .map_err(crate::shared::history::source_error)?;
    receipt["sourceObservationArtifacts"] =
        json!({"directory":"source-blocks","kind":"hash-pinned-discovery-provenance"});
    let mut history = History::new(&mut sources, genesis_hash, slot, &budget, 10000)?;
    let result = execute_hydrated(
        &request,
        config,
        &job,
        &budget,
        &mut receipt,
        &coordinator,
        |key, slot| hydrate_exact(&mut history, key, slot, &request.fixture),
    );
    let result = record_sources(result, &history, &job, &mut receipt);
    finish_job(coordinator.check_result(result), receipt, &job, started)
}

pub fn simulate_historical(
    request: HistoricalRequest,
    config: &Config,
    mut sources: CompositeSource,
    cancel: CancellationToken,
) -> Result<Value, Error> {
    let started = Instant::now();
    let budget = ExecutionBudget::new(Duration::from_millis(request.limits.timeout_ms), cancel)
        .map_err(worker_error)?;
    request.validate()?;
    sources
        .assert_genesis(&request.genesis_hash)
        .map_err(crate::shared::history::source_error)?;
    let (job, mut receipt, coordinator) = begin_job(
        &serde_json::to_value(&request)
            .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?,
        config,
        "source-reconstructed-replay/v1",
    )?;
    sources = sources
        .with_discovery_artifacts(&job.join("source-blocks"))
        .map_err(crate::shared::history::source_error)?;
    receipt["sourceObservationArtifacts"] =
        json!({"directory":"source-blocks","kind":"hash-pinned-discovery-provenance"});
    let mut history = History::new(
        &mut sources,
        request.genesis_hash.clone(),
        request.candidate["slot"]
            .as_u64()
            .ok_or_else(|| Error::new("INVALID_REQUEST", "historical slot required"))?,
        &budget,
        10000,
    )?;
    let result = (|| {
        let catalog =
            _1_resolve_runtime::load_catalog(&config.catalog_path, &config.catalog_sha256)?;
        _1_resolve_runtime::resolve_binding(
            &catalog,
            &request.family,
            &request.candidate,
            &request.runtime_binding,
        )?;
        budget.check().map_err(worker_error)?;
        let prepared = _2_prepare_state::source::reconstruct(&request, &mut history)?;
        coordinator.verify()?;
        let evidence =
            _5_finalize::write_json(&job.join("source-preparation.json"), &prepared.receipt)?;
        receipt["sourcePreparation"] = json!({"file":"source-preparation.json","sha256":evidence});
        receipt["reconstructedFromSources"] = json!(true);
        execute_hydrated(
            &prepared.request,
            config,
            &job,
            &budget,
            &mut receipt,
            &coordinator,
            |key, slot| hydrate_exact(&mut history, key, slot, &prepared.request.fixture),
        )
    })();
    let result = record_sources(result, &history, &job, &mut receipt);
    finish_job(coordinator.check_result(result), receipt, &job, started)
}
