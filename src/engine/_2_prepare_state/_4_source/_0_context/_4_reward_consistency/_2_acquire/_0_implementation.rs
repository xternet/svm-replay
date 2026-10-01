use super::*;

pub(in super::super::super) fn inactive_evidence(
    input: &HistoricalRequest,
    history: &mut History<'_>,
    slot: u64,
    block: &Value,
    proofs: &mut Vec<Value>,
) -> Result<Value, Error> {
    let key = stake::EPOCH_REWARDS;
    let observation = history.read(&json!({"kind":"account","genesisHash":history.genesis,
        "slot":slot,"pubkey":key,"phase":"end-slot"}))?;
    let mut roles = Roles::default();
    roles.sysvars.insert(key.into());
    let account = classify_account(observation["value"].clone(), key, slot, &roles)?;
    let account = repair(input, history, account, block, proofs)?;
    Ok(json!({"account":account,"evidenceHashes":observation["evidenceHashes"]}))
}

pub(in super::super::super) fn repair(
    input: &HistoricalRequest,
    history: &mut History<'_>,
    account: Value,
    block: &Value,
    proofs: &mut Vec<Value>,
) -> Result<Value, Error> {
    if account["pubkey"] != stake::EPOCH_REWARDS {
        return Ok(account);
    }
    let slot = input.runtime_binding["targetSlot"]
        .as_u64()
        .ok_or_else(|| invalid("reward target slot"))?;
    let executor = input.runtime_binding["executor"]["id"]
        .as_str()
        .ok_or_else(|| invalid("reward runtime"))?;
    match validate(std::slice::from_ref(&account), executor, slot, block) {
        Ok(()) => return Ok(account),
        Err(error)
            if error
                .details
                .as_ref()
                .is_some_and(|d| d["reason"] == "inconsistent-epoch-rewards") => {}
        Err(error) => return Err(error),
    }
    let unsupported = |s: &str| Error::new("UNSUPPORTED_BANK_INPUT", s);
    if history.genesis != "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d" {
        return Err(unsupported("reward recovery registry is mainnet-specific"));
    }
    let cursor = history.read(&json!({"kind":"account","genesisHash":history.genesis,
        "pubkey":stake::EPOCH_REWARDS,"slot":slot,"phase":"last-write-at-or-before-slot"}))?;
    let write = &cursor["value"];
    let write_slot = write["sourceSlot"]
        .as_u64()
        .ok_or_else(|| invalid("reward write slot"))?;
    if write_slot > slot {
        return Err(unsupported("reward write is in the future"));
    }
    let registry = Registry::bundled()?;
    if registry.profile(slot)?.profile_id.as_str()
        != input.runtime_binding["runtimeProfileId"]
            .as_str()
            .ok_or_else(|| invalid("reward profile"))?
    {
        return Err(unsupported("reward registry differs from target binding"));
    }
    let catalog = registry.lifecycle_catalog();
    if registry.profile(slot)?.executor_source_id != executor {
        return Err(unsupported("reward executor differs from registry"));
    }
    let mut covered = write_slot;
    for profile in array(&catalog["profiles"], "reward profiles")? {
        let first = profile["earliestSlot"]
            .as_u64()
            .ok_or_else(|| invalid("profile start"))?;
        let last = profile["latestSlot"]
            .as_u64()
            .ok_or_else(|| invalid("profile end"))?;
        if first <= slot
            && last >= write_slot
            && !matches!(
                profile["executorSourceId"].as_str(),
                Some("litesvm-v0.14.0-pr402-agave-4.1.2" | "litesvm-v0.16.0-agave-4.2.1")
            )
        {
            return Err(unsupported(
                "reward interval includes an unreviewed runtime",
            ));
        }
        if first <= slot && last >= write_slot {
            if first > covered {
                return Err(unsupported("reward interval has a runtime coverage gap"));
            }
            covered = last.saturating_add(1);
        }
    }
    if covered <= slot {
        return Err(unsupported("reward interval coverage is incomplete"));
    }
    let write_executor = &registry.profile(write_slot)?.executor_source_id;
    let observed_block = history.discover_block(write_slot)?;
    let raw = STANDARD
        .decode(
            observed_block["value"]["rawBase64"]
                .as_str()
                .ok_or_else(|| invalid("reward write block bytes"))?,
        )
        .map_err(|_| invalid("reward block encoding"))?;
    let envelope = svm_replay_protocol::parse_json(&raw)?;
    let mut roles = Roles::default();
    roles.sysvars.extend([CLOCK.into(), RENT.into()]);
    let write_clock = history.account(CLOCK, write_slot, &roles)?;
    let target_clock = history.account(CLOCK, slot, &roles)?;
    let rent = history.account(RENT, write_slot, &roles)?;
    let features = array(&catalog["features"], "reward features")?;
    if features.iter().any(|f| {
        f["id"] == "a1penGLz8Vm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP"
            && f["activationSlot"].as_u64().is_some_and(|s| s <= slot)
    }) {
        return Err(unsupported(
            "reward interval includes unreviewed consensus features",
        ));
    }
    let sharing = features.iter().any(|f| {
        f["id"] == "B1ockRevenueSharing111111111111111111111111"
            && f["activationSlot"]
                .as_u64()
                .is_some_and(|s| s <= write_slot)
    });
    let ctx = _1_terminal::TerminalContext {
        slot,
        write_slot,
        write_height: envelope["result"]["blockHeight"]
            .as_u64()
            .ok_or_else(|| invalid("reward write height"))?,
        height: block["blockHeight"]
            .as_u64()
            .ok_or_else(|| invalid("reward target height"))?,
        write_epoch: _3_fields::epoch(&write_clock, write_slot)?,
        epoch: _3_fields::epoch(&target_clock, slot)?,
        executor: write_executor,
        block_revenue_sharing: sharing,
        rent_minimum: _3_fields::rent_minimum(&rent, write_slot)?,
    };
    let recovered = _1_terminal::recover(&account, write, &ctx)?;
    proofs.push(json!({"schema":"svm-replay-terminal-epoch-rewards/v1","slot":slot,
        "writeSlot":write_slot,"writeHeight":ctx.write_height,"epoch":ctx.epoch,
        "writeExecutorSourceId":write_executor,"blockRevenueSharing":sharing,
        "rentMinimum":ctx.rent_minimum.to_string(),"rawAccount":account,"archivedWrite":write,
        "derivedAccountSha256":Digest::of(canonical_json(&recovered)),
        "registrySha256":Digest::of(canonical_json(&catalog)),"writeBlockSha256":Digest::of(raw),
        "writeClock":write_clock,"targetClock":target_clock,"writeRent":rent,
        "cursorEvidenceHashes":cursor["evidenceHashes"],"blockEvidenceHashes":observed_block["evidenceHashes"]}));
    Ok(recovered)
}
