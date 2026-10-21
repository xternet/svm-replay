use super::*;

/// Acquires a canonical initialized account scope. It never selects or executes
/// an archived transaction and never substitutes a transaction control.
pub fn prepare(input: &InitializationRequest, history: &mut History<'_>) -> Result<Value, Error> {
    let block = input.validate()?;
    if history.genesis != input.genesis_hash || history.target_slot != input.slot {
        return Err(invalid("initialization history differs from request"));
    }
    let (raw, observed) = history.block(&input.block_sha256)?;
    if observed != block {
        return Err(invalid("observed initialization block differs"));
    }
    let mut images = _1_images::acquire(input, &block, history)?;
    let features = input.runtime_binding["features"]
        .as_array()
        .ok_or_else(|| invalid("features missing"))?
        .iter()
        .map(|f| f["id"].clone())
        .collect::<Vec<_>>();
    let stake_input = json!({"accounts":images.accounts,"slot":input.slot,"parentSlot":block["result"]["parentSlot"],
        "blockSourceHash":input.block_sha256,"envelope":block,
        "runtime":{"executorSourceId":input.runtime_binding["executor"]["id"],"activeExecutionFeatureIds":features},
        "phaseContext":{"clock":images.clock,"rent":images.accounts.iter().find(|a| a["pubkey"] == "SysvarRent111111111111111111111111111111111")}});
    let initialized_stakes = stake::prepare_stake_initialization(&stake_input)?;
    images.accounts = initialized_stakes["accounts"]
        .as_array()
        .ok_or_else(|| invalid("initialized stake images missing"))?
        .clone();
    let mut bank =
        context::derive_initialized_bank_context(&block, input.slot, input.block_sha256.as_str())?;
    let mut generic = vec![sysvars::bind_exact_generic_sysvar(
        &images.clock,
        input.slot,
    )?];
    for account in &images.accounts {
        if account["role"] == "sysvar"
            && sysvars::GENERIC_SYSVARS
                .iter()
                .any(|id| account["pubkey"] == *id)
        {
            generic.push(sysvars::bind_exact_generic_sysvar(account, input.slot)?);
        }
    }
    bank["requiredRuntimeSysvars"] = json!(generic
        .iter()
        .map(|v| json!({"pubkey":v["pubkey"],"requirement":"tracked-generic-sysvar-context-v1"}))
        .collect::<Vec<_>>());
    bank["derivationHash"] = json!(context::historical_bank_context_hash(&bank)?);
    let mut runtime = json!({"binding":input.runtime_binding,"bankContext":bank,"clockDataHash":generic[0]["dataSha256"],
        "lastRestartSlot":images.restart["binding"],"bankInitializedSysvars":images.initialized["bindings"],
        "genericSysvars":generic,"afterSlot":input.after_slot,"programMigrationProofs":images.migrations["proofs"],
        "stakeInitializationProofs":initialized_stakes["proofs"]});
    runtime["profileHash"] = json!(crate::_2_prepare_state::discovery::runtime_profile_hash(
        &runtime
    )?);
    let fixture = json!({"schema":"svm-historical-initialized-slot/v1","caseId":input.request_id,
        "target":{"kind":"initialized-slot","targetSlot":input.slot,"index":0},
        "runtime":runtime,"clock":images.clock,"accounts":images.accounts,"endAccounts":[],"accountOverride":null});
    migration::assert_program_migration_bindings(&fixture)?;
    snapshots::assert_bound_context(&fixture)?;
    sysvars::assert_bank_initialized_sysvar_bindings(
        fixture["accounts"]
            .as_array()
            .ok_or_else(|| invalid("initialized images absent"))?,
        input.slot,
        fixture["runtime"]["bankInitializedSysvars"]
            .as_array()
            .ok_or_else(|| invalid("initialized bindings absent"))?,
    )?;
    Ok(
        json!({"schema":"svm-historical-initialization-prepared/v1","request":input,
        "fixture":fixture,"blockSha256":Digest::of(raw),"sourceObservations":history.sources.observations(),
        "original":null,"originalControl":{"status":"NOT_APPLICABLE","reason":"no archived transaction is executed at slot initialization"}}),
    )
}
