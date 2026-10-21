use super::*;
use crate::shared::progress::{self, Phase};

pub fn reconstruct(
    input: &HistoricalRequest,
    history: &mut History<'_>,
) -> Result<Prepared, Error> {
    progress::phase(Phase::ResolvingDependencies);
    input.validate()?;
    if input.genesis_hash != history.genesis
        || input.candidate["slot"].as_u64() != Some(history.target_slot)
    {
        return Err(Error::new(
            "SOURCE_CONTEXT_MISMATCH",
            "request and history context differ",
        ));
    }
    let source_hash = Digest::new(
        input.candidate["blockSourceHash"]
            .as_str()
            .ok_or_else(|| invalid("block source hash"))?,
    )?;
    let (raw_bytes, envelope) = history.block(&source_hash)?;
    let a = analyze(input, &envelope)?;
    let original = &a.transactions[a.index as usize];
    let binding = &input.runtime_binding;
    let supplemental = bank_inputs(input, history, &a, &source_hash)?;
    let changes = match &input.requested_account_overrides {
        Some(value) => overrides::validate_requested_overrides(value)?,
        None => Vec::new(),
    };
    let overridden_lookup = changes
        .iter()
        .any(|entry| original.address_table_accounts.contains(&entry.pubkey));
    let original_wire = history.transaction(&original.signature)?;
    svm_replay_protocol::transaction::assert_historical(&original_wire, &a.raw[a.index as usize])?;
    let requested = what_if::requested_transactions(
        input,
        &a,
        &original_wire,
        overridden_lookup,
        &changes,
        history,
    )?;
    let merged = what_if::merged_summary(&requested)?;
    let requested_summary = merged.as_ref().unwrap_or(original);
    let dependency_target = what_if::dependency_target(input, original, merged.as_ref())?;
    let mut transactions = a.transactions.clone();
    transactions[a.index as usize] = dependency_target.clone();
    let mut migration_input = json!({"slot":a.slot,"parentSlot":a.parent,"blockSourceHash":source_hash,
        "envelope":envelope,"runtime":binding,"requestedPubkeys":[]});
    if let Some(evidence) = supplemental.get("programMigrationEvidence") {
        migration_input["evidence"] = evidence.clone();
    }
    let mut migrations = migration::program_migration_context(&migration_input)?;
    let mut inspected = BTreeSet::new();
    let mut headers =
        _11_program_headers::Recovery::new(&a, &transactions, binding, &envelope["result"]);
    let closure = dependencies::compute_program_data_fixed_point(&transactions, a.index, |keys| {
        inspected.extend(keys.iter().cloned());
        migration_input["requestedPubkeys"] = json!(inspected);
        migrations = migration::program_migration_context(&migration_input)?;
        let initialized = array(&migrations["initialized"], "migration images")?;
        let unresolved = keys
            .iter()
            .filter(|key| !initialized.iter().any(|v| v["pubkey"] == **key))
            .cloned()
            .collect::<Vec<_>>();
        let links = headers.links(history, &unresolved)?;
        let resolved =
            migration::resolve_program_migration_overlays(&migrations, keys, &json!(links))?;
        serde_json::from_value(resolved)
            .map_err(|e| invalid(format!("migration program links:{e}")))
    })?;
    let credits = _10_credit_recovery::recover(input, &a, history, &transactions, closure)?;
    let closure = credits.closure;
    progress::earlier_total(closure.selected_indices.len() as u64, true);
    let selected = closure
        .selected_indices
        .iter()
        .map(|i| a.transactions[*i as usize].clone())
        .collect::<Vec<_>>();
    let mut included = selected.clone();
    included.push(dependency_target.clone());
    let mut execution = closure.selected_indices.clone();
    execution.push(a.index);
    let (requirements, runtime_resolvable, roles) =
        runtime_requirements(&a, &execution, &requested, &included, &closure)?;
    let initialized = array(&migrations["initialized"], "migration images")?;
    let parent_keys = parent_keys(&closure, initialized, &runtime_resolvable)?;
    // Read neutral records first: lifecycle proofs determine which absent images
    // are legitimate creation inputs, before assigning required program/ALT roles.
    progress::phase(Phase::FetchingHistoricalAccounts);
    let mut parent_records = history.accounts(&parent_keys, a.parent, &Roles::default())?;
    headers.apply(&mut parent_records)?;
    _9_implicit_loaders::include_loaders(&mut parent_records, |key| {
        history.account(key, a.parent, &Roles::default())
    })?;
    migration::validate_program_migration_parents(&migrations, &parent_records)?;
    let created = requested::prepare_created_alt_parents(requested::CreatedAltInput {
        records: &parent_records,
        roles: &roles,
        parent_slot: a.parent,
        target_slot: a.slot,
        target_index: a.index,
        selected: &selected,
        raw_transactions: &a.raw,
        requested_table_keys: &requested_summary.address_table_accounts,
        overrides: &changes,
    })?;
    let mut loader_evidence = execution
        .iter()
        .map(|&index| lifecycle::IncludedTransaction {
            index,
            raw: a.raw[index as usize].clone(),
        })
        .collect::<Vec<_>>();
    for value in &requested {
        loader_evidence.push(lifecycle::IncludedTransaction {
            index: a.index,
            raw: value.raw.clone(),
        });
    }
    let parents = parent_records
        .iter()
        .filter(|v| !initialized.iter().any(|i| i["pubkey"] == v["pubkey"]))
        .cloned()
        .collect::<Vec<_>>();
    let mut parent = lifecycle::build_inspected_loader_parent_accounts(
        &parents,
        a.parent,
        &created.parent_roles,
        &loader_evidence,
        a.index,
    )?;
    parent.extend(initialized.iter().cloned());
    overrides::assert_override_membership(
        &changes,
        &requested_summary.declared_accounts,
        &requested_summary.address_table_accounts,
        &parent,
    )?;
    let executable = if !requested.is_empty()
        || input.requested_account_overrides.is_some()
        || !input.extra_accounts.is_empty()
    {
        parent
            .iter()
            .filter(|v| {
                v["presence"] == "present"
                    && v["executable"] == true
                    && dependency_target
                        .declared_accounts
                        .iter()
                        .any(|key| v["pubkey"] == *key)
            })
            .map(|v| {
                v["pubkey"]
                    .as_str()
                    .ok_or_else(|| invalid("executable pubkey"))
                    .map(str::to_owned)
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let programs = unique(
        roles
            .programs
            .iter()
            .cloned()
            .chain(executable)
            .chain(what_if::override_programs(input)),
    );
    let inspected_images = what_if::inventory_images(&parent, &changes, input)?;
    let extra = unique(
        imports::dedicated_inputs(&programs, &parent)?
            .into_iter()
            .chain(imports::dedicated_inputs(&programs, &inspected_images)?),
    )
    .into_iter()
    .filter(|key| !parent.iter().any(|v| v["pubkey"] == *key))
    .collect::<Vec<_>>();
    let mut extra_roles = roles.clone();
    extra_roles.sysvars.extend(extra.iter().cloned());
    parent.extend(history.accounts(&extra, a.parent, &extra_roles)?);
    let loader_context = lifecycle::HistoricalLoaderContext {
        parent_slot: a.parent,
        accounts: parent.clone(),
        included_transactions: loader_evidence,
    };
    let mut original_included = selected.clone();
    original_included.push(original.clone());
    let lifecycle_indices = lifecycle::assert_program_lifecycle_supported(
        &original_included,
        Some(&a.raw),
        Some(&loader_context),
    )?;
    for value in &requested {
        let mut raw = a.raw.clone();
        raw[a.index as usize] = value.raw.clone();
        lifecycle::assert_program_lifecycle_supported(
            std::slice::from_ref(&value.summary),
            Some(&raw),
            Some(&loader_context),
        )?;
    }
    _10_credit_recovery::apply(&mut parent, &credits.images)?;
    context::finish(
        input,
        history,
        context::Boundary {
            a: &a,
            envelope: &envelope,
            raw_bytes: &raw_bytes,
            closure: &closure,
            requirements: &requirements,
            roles: &roles,
            created: &created,
            programs: &programs,
            changes: &changes,
            migrations: &migrations,
            supplemental: &supplemental,
            parent,
            original_wire: &original_wire,
            selected: &selected,
            dependency_transactions: &transactions,
            lifecycle_indices,
            credit_proofs: &credits.proofs,
            header_proofs: &headers.proofs,
        },
    )
}
