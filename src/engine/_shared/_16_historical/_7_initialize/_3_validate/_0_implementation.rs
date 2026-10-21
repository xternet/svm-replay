use super::*;

/// Recompute initialized Bank metadata and bind every admitted image to the
/// retained exact-slot source journal. Hashes identify evidence, not consensus.
pub(in super::super::super) fn validate(packet: &Value) -> Result<InitializationRequest, Error> {
    if packet["schema"] != "svm-historical-initialization-prepared/v1"
        || packet["original"] != Value::Null
        || packet["originalControl"]["status"] != "NOT_APPLICABLE"
    {
        return Err(invalid("invalid initialization packet/control"));
    }
    let input: InitializationRequest = serde_json::from_value(packet["request"].clone())
        .map_err(|e| invalid(format!("initialization request: {e}")))?;
    let block = input.validate()?;
    let parent = position(&block, input.after_slot, input.slot)?;
    let fixture = &packet["fixture"];
    if fixture["schema"] != "svm-historical-initialized-slot/v1"
        || fixture["target"] != json!({"kind":"initialized-slot","targetSlot":input.slot,"index":0})
        || fixture["runtime"]["binding"] != input.runtime_binding
        || fixture["runtime"]["afterSlot"] != input.after_slot
        || fixture["caseId"] != input.request_id
        || fixture["endAccounts"] != json!([])
        || packet["blockSha256"] != input.block_sha256.as_str()
    {
        return Err(invalid("initialized fixture position/runtime differs"));
    }
    for name in ["whatIf", "accountOverride", "requestedAccountOverrides"] {
        if fixture.get(name).is_some_and(|v| !v.is_null()) {
            return Err(invalid("initialized source contains hypothetical changes"));
        }
    }
    let accounts = fixture["accounts"]
        .as_array()
        .ok_or_else(|| invalid("initial images missing"))?;
    let observations = packet["sourceObservations"]
        .as_array()
        .ok_or_else(|| invalid("initialization source journal missing"))?;
    let mut seen = BTreeSet::new();
    for account in accounts.iter().chain(std::iter::once(&fixture["clock"])) {
        let id = account["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("initial image key missing"))?;
        if !seen.insert(id) {
            return Err(invalid("duplicate initial image"));
        }
        let slot = if id == CLOCK || sysvars::BANK_INITIALIZED_SYSVARS.contains(&id) {
            input.slot
        } else {
            parent
        };
        let mut roles = Roles::default();
        if sysvars::GENERIC_SYSVARS.contains(&id) {
            roles.sysvars.insert(id.to_owned());
        }
        history::classify_account(account.clone(), id, slot, &roles)?;
        let mut semantic = account.clone();
        semantic
            .as_object_mut()
            .ok_or_else(|| invalid("initial image object missing"))?
            .remove("role");
        let hash = Digest::of(format!("m11-source-value/v1\n{}\n", semantic));
        if !observations.iter().any(|o| o["status"] == "AVAILABLE" && o["valueSha256"] == hash.as_str()
            && o["query"] == json!({"kind":"account","genesisHash":input.genesis_hash,"slot":slot,"pubkey":id,"phase":"end-slot"}))
        { return Err(invalid(format!("initialized image has no exact source witness: {id}"))); }
    }
    sysvars::validate_target_clock(&fixture["clock"], input.slot)?;
    let mut bank =
        context::derive_initialized_bank_context(&block, input.slot, input.block_sha256.as_str())?;
    let generic = fixture["runtime"]["genericSysvars"]
        .as_array()
        .ok_or_else(|| invalid("generic bindings missing"))?;
    let expected = std::iter::once(&fixture["clock"])
        .chain(
            accounts
                .iter()
                .filter(|a| sysvars::GENERIC_SYSVARS.iter().any(|id| a["pubkey"] == *id)),
        )
        .map(|a| sysvars::bind_exact_generic_sysvar(a, input.slot))
        .collect::<Result<Vec<_>, _>>()?;
    if *generic != expected {
        return Err(invalid("generic initialization bindings differ"));
    }
    bank["requiredRuntimeSysvars"] = json!(generic
        .iter()
        .map(|v| json!({"pubkey":v["pubkey"],"requirement":"tracked-generic-sysvar-context-v1"}))
        .collect::<Vec<_>>());
    bank["derivationHash"] = json!(context::historical_bank_context_hash(&bank)?);
    if bank != fixture["runtime"]["bankContext"]
        || fixture["runtime"]["profileHash"]
            != crate::_2_prepare_state::discovery::runtime_profile_hash(&fixture["runtime"])?
                .as_str()
    {
        return Err(invalid(
            "initialized Bank derivation differs from source metadata",
        ));
    }
    let bindings = fixture["runtime"]["bankInitializedSysvars"]
        .as_array()
        .ok_or_else(|| invalid("Bank sysvar bindings missing"))?;
    sysvars::assert_bank_initialized_sysvar_bindings(accounts, input.slot, bindings)?;
    snapshots::assert_bound_context(fixture)?;
    Ok(input)
}
