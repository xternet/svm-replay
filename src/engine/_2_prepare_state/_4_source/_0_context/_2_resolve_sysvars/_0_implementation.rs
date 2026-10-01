use super::*;

pub(super) fn supports_recent_alt(executor: &str) -> bool {
    matches!(
        executor,
        "litesvm-v0.6.1-agave-2.2.20"
            | "litesvm-v0.7.1-agave-2.3.9"
            | "litesvm-v0.8.2-agave-3.0.10"
            | "litesvm-v0.12.0-agave-3.1.11"
            | "litesvm-v0.13.1-agave-4.0.0"
            | "litesvm-v0.14.0-pr402-agave-4.1.2"
            | "litesvm-v0.16.0-agave-4.2.1"
    )
}

pub(in super::super) fn resolve_sysvars(
    history: &mut History<'_>,
    boundary: &Boundary<'_>,
    accounts: &mut Vec<Value>,
    clock: &Value,
    generic_bindings: &[Value],
    override_bindings: &[Value],
    override_context: &override_sysvars::OverrideSysvarContext<'_>,
    executor: &str,
    sysvar_roles: &mut Roles,
) -> Result<(Vec<Value>, Vec<Value>, Value), Error> {
    let Boundary {
        a,
        created,
        roles,
        changes,
        requirements,
        closure,
        ..
    } = boundary;
    let preflight = if created.parent_roles.address_tables.is_empty() {
        json!({"status":"NOT_REQUIRED","count":0})
    } else {
        alt::preflight_active_address_tables(&accounts)?
    };
    let mut generic = vec![sysvars::bind_exact_generic_sysvar(&clock, a.slot)?];
    generic.extend(generic_bindings.iter().cloned());
    let target_tables = roles.address_tables.iter().cloned().collect::<Vec<_>>();
    let mut old_deactivation = false;
    if preflight["status"] == "REQUIRES_SLOT_HASHES" {
        for table in array(&preflight["tables"], "deactivated tables")? {
            let slot = table["deactivationSlot"]
                .as_str()
                .ok_or_else(|| invalid("ALT deactivation slot missing"))?
                .parse::<u64>()
                .map_err(|_| invalid("ALT deactivation slot malformed"))?;
            let age = a.slot.checked_sub(slot).ok_or_else(|| {
                Error::new(
                    "UNSUPPORTED_ALT_LIFECYCLE",
                    "historical ALT deactivation is after target",
                )
            })?;
            old_deactivation |= age >= 512;
        }
    }
    let needs_alt_hashes =
        overrides::overridden_lookup_needs_slot_hashes(changes, &target_tables, a.slot)?
            || preflight["status"] == "REQUIRES_SLOT_HASHES"
                && (!supports_recent_alt(executor) || old_deactivation);
    if needs_alt_hashes
        && !generic
            .iter()
            .any(|b| b["pubkey"] == instructions::SLOT_HASHES)
    {
        sysvar_roles
            .sysvars
            .insert(instructions::SLOT_HASHES.into());
        let hashes = history.account(instructions::SLOT_HASHES, a.slot, &sysvar_roles)?;
        generic.push(sysvars::bind_exact_generic_sysvar(&hashes, a.slot)?);
        accounts.retain(|v| v["pubkey"] != instructions::SLOT_HASHES);
        accounts.push(hashes);
    }
    // Declared account reads happen before a generic-syscall guard can hydrate
    // them. Fetch exact supported inputs now; unrecorded history remains explicit.
    for key in strings(&requirements["exactAccountRequired"])? {
        if !sysvars::GENERIC_SYSVARS.contains(&key.as_str())
            || generic.iter().any(|b| b["pubkey"] == key)
        {
            continue;
        }
        sysvar_roles.sysvars.insert(key.clone());
        let account = history.account(&key, a.slot, &sysvar_roles)?;
        generic.push(sysvars::bind_exact_generic_sysvar(&account, a.slot)?);
        accounts.retain(|value| value["pubkey"] != key);
        accounts.push(account);
    }
    // The Clock is already exact and always declared for guarded execution.
    let mut seen = BTreeSet::new();
    for value in &generic {
        let key = value["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("generic binding key"))?;
        if !seen.insert(key) {
            return Err(invalid("duplicate generic sysvar binding"));
        }
    }
    let parent_overrides = override_sysvars::proven_parent_override_sysvars(
        &accounts,
        override_bindings,
        &override_context,
    )?;
    let exact_needed = unique(
        closure
            .excluded_dependency_accounts
            .iter()
            .cloned()
            .chain(strings(&requirements["exactAccountRequired"])?),
    );
    let missing = exact_needed
        .into_iter()
        .filter(|id| {
            !roles.excluded.contains(id)
                && !parent_overrides.contains(id)
                && !generic.iter().any(|b| b["pubkey"] == *id)
        })
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(Error::new(
            "UNSUPPORTED_EXACT_SYSVAR_UNAVAILABLE",
            missing.join(","),
        ));
    }
    let recent = if preflight["status"] == "NOT_REQUIRED" {
        Vec::new()
    } else {
        alt::build_recent_address_table_proofs(
            &accounts,
            a.slot,
            a.parent,
            generic
                .iter()
                .any(|b| b["pubkey"] == instructions::SLOT_HASHES),
        )?
    };
    if !recent.is_empty() && !supports_recent_alt(executor) {
        return Err(Error::new(
            "UNSUPPORTED_ALT_LIFECYCLE",
            "recent deactivation proof is unavailable for this unreviewed executor",
        ));
    }
    let address_tables = if recent.is_empty() {
        preflight.clone()
    } else {
        json!({"status":"SUPPORTED_RECENT_DEACTIVATION","count":roles.address_tables.len(),"proofs":recent})
    };
    Ok((generic, recent, address_tables))
}
