use super::*;

pub(in super::super::super) fn finish(
    input: &HistoricalRequest,
    history: &mut History<'_>,
    mut boundary: Boundary<'_>,
) -> Result<Prepared, Error> {
    let parent = std::mem::take(&mut boundary.parent);
    let Boundary {
        a,
        envelope,
        raw_bytes,
        closure,
        requirements,
        roles: _,
        created,
        programs,
        changes,
        migrations,
        supplemental,
        parent: _,
        original_wire,
        selected,
        dependency_transactions,
        ref lifecycle_indices,
        credit_proofs,
        header_proofs,
    } = boundary;
    let binding = &input.runtime_binding;
    let source_hash = Digest::of(raw_bytes);
    let instructions_sysvar =
        instructions::analyze_instructions_sysvar(&a.raw, &closure.selected_indices, a.index)?;
    let features = json!(array(&binding["features"], "runtime features")?
        .iter()
        .map(|f| f["id"].clone())
        .collect::<Vec<_>>());
    let executor = binding["executor"]["id"]
        .as_str()
        .ok_or_else(|| invalid("executor missing"))?;
    let override_context = override_sysvars::OverrideSysvarContext {
        parent_slot: a.parent,
        target_slot: a.slot,
        executor_source_id: executor,
        active_execution_feature_ids: &features,
    };
    let baselines = override_sysvars::hydrate_override_sysvar_baselines(
        changes,
        &parent,
        &override_context,
        history,
    )?;
    let parent = baselines.accounts;
    let mut reward_proofs = Vec::new();
    let (restart, bank_initialized, mut accounts, clock, mut sysvar_roles) = initialize_sysvars(
        input,
        history,
        &parent,
        a.slot,
        &envelope["result"],
        &mut reward_proofs,
    )?;
    super::super::_4_reward_consistency::validate(
        &accounts,
        executor,
        a.slot,
        &envelope["result"],
    )?;
    let mut initialized_snapshot = None;
    let stake_proofs;
    if let Some(evidence) = supplemental.get("initializedStakeEvidence") {
        let initialized = snapshots::prepare_exact_initialized_stakes(
            &json!({"accounts":accounts,"slot":a.slot,"parentSlot":a.parent,
            "blockhash":a.blockhash,"blockSourceHash":source_hash,"runtime":binding,"evidence":evidence}),
        )?;
        accounts = array(&initialized["accounts"], "exact initialized stake accounts")?.clone();
        initialized_snapshot = Some(initialized["snapshot"].clone());
        stake_proofs = Vec::new();
    } else {
        let mut request = json!({"accounts":accounts,"slot":a.slot,"parentSlot":a.parent,"envelope":envelope,
            "blockSourceHash":source_hash,"runtime":{"executorSourceId":executor,"activeExecutionFeatureIds":features},
            "phaseContext":{"clock":clock,"rent":required(&accounts,RENT)?}});
        if let Some(evidence) = supplemental.get("initializationEvidence") {
            request["evidence"] = evidence.clone();
        } else if stake::supports_inactive_rewards(executor)
            && accounts
                .iter()
                .any(|account| account["owner"] == "Stake11111111111111111111111111111111111111")
        {
            request["inactiveRewards"] = super::super::_4_reward_consistency::inactive_evidence(
                input,
                history,
                a.slot,
                &envelope["result"],
                &mut reward_proofs,
            )?;
        }
        let initialized = stake::prepare_stake_initialization(&request)?;
        accounts = array(&initialized["accounts"], "initialized stake accounts")?.clone();
        stake_proofs = array(&initialized["proofs"], "stake proofs")?.clone();
    }
    assert_account_coverage(&accounts, &a.raw[..=a.index as usize], a.parent)?;
    let (generic, recent, address_tables) = resolve_sysvars(
        history,
        &boundary,
        &mut accounts,
        &clock,
        &baselines.generic_sysvars,
        &baselines.bindings,
        &override_context,
        executor,
        &mut sysvar_roles,
    )?;
    let original = &a.transactions[a.index as usize];
    let mut bank_context = bank_context::derive_historical_bank_context(
        envelope,
        &json!({"slot":a.slot,"parentSlot":a.parent,
        "blockhash":a.blockhash,"index":a.index,"signature":original.signature}),
        source_hash.as_str(),
        &closure
            .selected_indices
            .iter()
            .map(|i| *i as usize)
            .collect::<Vec<_>>(),
    )?;
    let required = bank_context["requiredRuntimeSysvars"]
        .as_array_mut()
        .ok_or_else(|| invalid("derived requirements missing"))?;
    if requirements["recentBlockhashes"]["runtimeResolvable"] == true
        && !required.iter().any(|v| v["pubkey"] == RECENT_BLOCKHASHES)
    {
        required.push(
            json!({"pubkey":RECENT_BLOCKHASHES,"requirement":"nonce-advance-nonempty-cache-v1"}),
        );
    }
    required.extend(
        generic.iter().map(
            |b| json!({"pubkey":b["pubkey"],"requirement":"tracked-generic-sysvar-context-v1"}),
        ),
    );
    bank_context["derivationHash"] =
        json!(bank_context::historical_bank_context_hash(&bank_context)?);
    let mut runtime = json!({"binding":binding,"bankContext":bank_context,"clockDataHash":sysvars::bind_exact_generic_sysvar(&clock,a.slot)?["dataSha256"],
        "lastRestartSlot":restart["binding"],"bankInitializedSysvars":bank_initialized["bindings"],"genericSysvars":generic});
    for (name, values) in [
        ("recentAddressTables", recent),
        ("stakeInitializationProofs", stake_proofs),
        (
            "programMigrationProofs",
            array(&migrations["proofs"], "migration proofs")?.clone(),
        ),
    ] {
        if !values.is_empty() {
            runtime[name] = json!(values);
        }
    }
    if let Some(value) = initialized_snapshot {
        runtime["initializedStakeSnapshot"] = value;
    }
    if !credit_proofs.is_empty() {
        runtime["systemCreditPreloads"] = json!(credit_proofs);
    }
    if !header_proofs.is_empty() {
        runtime["programHeaderProofs"] = json!(header_proofs);
    }
    let mut wires = Vec::new();
    for tx in selected {
        let bytes = history.transaction(&tx.signature)?;
        svm_replay_protocol::transaction::assert_historical(&bytes, &a.raw[tx.index as usize])?;
        wires.push(bytes);
    }
    let fee_effects =
        fees::derive_omitted_fee_effects(&a.raw, dependency_transactions, a.index, closure)?;
    let mut indices = closure.selected_indices.clone();
    indices.push(a.index);
    let end_keys = fees::eligible_end_accounts(&a.transactions, &indices)?;
    let end_accounts = history.accounts(&end_keys, a.slot, &Roles::default())?;
    let mut fixture = json!({"schema":"svm-simulate-m6-fixture/v1","caseId":input.candidate["id"],
        "target":{"targetSlot":a.slot,"parentSlot":a.parent,"index":a.index,"signature":original.signature,
            "prefixIndices":closure.selected_indices,"prefixSignatures":selected.iter().map(|t|&t.signature).collect::<Vec<_>>(),
            "prefixTransactionsBase64":wires,"transactionBase64":original_wire,"replacementTransactionBase64":input.replacement_transaction_base64,
            "evidenceAccounts":original.writable_accounts,"prefixEvidenceAccounts":selected.iter().map(|t|json!({"index":t.index,"pubkeys":t.writable_accounts})).collect::<Vec<_>>(),
            "omittedFeeEffects":fee_effects["omittedFeeEffects"],"omittedFeeEffectsHash":fee_effects["omittedFeeEffectsHash"]},
        "runtime":runtime,"clock":clock,"accounts":accounts,"endAccounts":end_accounts,"accountOverride":null});
    if let Some(value) = &input.requested_account_overrides {
        fixture["requestedAccountOverrides"] = value.clone();
    }
    if let Some(evidence) = supplemental.get("epochStakeEvidence") {
        fixture["runtime"]["epochStakes"] =
            snapshots::bind_epoch_stake_evidence(&json!({"evidence":evidence,"fixture":fixture}))?;
    }
    fixture["runtime"]["profileHash"] = json!(runtime_profile_hash(&fixture["runtime"])?);
    sysvars::assert_bank_initialized_sysvar_bindings(
        &accounts,
        a.slot,
        array(&bank_initialized["bindings"], "Bank sysvar bindings")?,
    )?;
    snapshots::assert_bound_context(&fixture)?;
    migration::assert_program_migration_bindings(&fixture)?;
    let initialized_ids = array(&bank_initialized["bindings"], "Bank sysvar bindings")?
        .iter()
        .map(|b| {
            b["pubkey"]
                .as_str()
                .ok_or_else(|| invalid("binding pubkey"))
                .map(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inventory = imports::inspect(
        programs,
        &accounts,
        &initialized_ids,
        fixture["runtime"].get("epochStakes").is_some(),
    )?;
    let requested_inventory = imports::inspect(
        programs,
        &overrides::requested_image_inspection(&accounts, changes)?,
        &initialized_ids,
        fixture["runtime"].get("epochStakes").is_some(),
    )?;
    let observations = history.sources.observations();
    let observations_hash = Digest::of(canonical_json(&json!(observations)));
    let mut evidence = BTreeSet::from([source_hash.clone(), observations_hash.clone()]);
    for observation in &observations {
        for hash in strings(&observation["evidenceHashes"])? {
            evidence.insert(Digest::new(hash)?);
        }
    }
    let receipt = json!({"schema":"svm-replay-source-preparation/v1","status":"PREPARED","currentStateFallback":false,
        "genesisHash":input.genesis_hash,"slot":a.slot,"parentSlot":a.parent,"transactionIndex":a.index,
        "sourceObservations":observations,"sourceObservationsSha256":observations_hash,"closure":closure,"runtimeRequirements":requirements,
        "instructionsSysvar":instructions_sysvar,"systemCreditPreloads":credit_proofs,"terminalEpochRewards":reward_proofs,
        "lastRestartProvenance":restart["provenance"],"addressTables":address_tables,"createdAddressTables":created.created_tables,
        "overrideSysvarBaselines":baselines.bindings,"lifecycleIndices":lifecycle_indices,"inventory":inventory,"requestedInventory":requested_inventory,
        "certifiedSimulation":false,"trust":"Explicit historical source evidence; hashes are not consensus proofs."});
    let request = PreparedRequest {
        schema: "svm-replay-prepared/v1".into(),
        request_id: input.request_id.clone(),
        family: input.family.clone(),
        candidate: input.candidate.clone(),
        fixture,
        raw_block_base64: STANDARD.encode(raw_bytes),
        block_sha256: source_hash,
        source_evidence_hashes: evidence.into_iter().collect(),
        metadata_policy: input.metadata_policy,
        limits: input.limits.clone(),
    };
    request.validate()?;
    super::super::super::super::validate_boundary(&request, envelope)?;
    Ok(Prepared { request, receipt })
}
