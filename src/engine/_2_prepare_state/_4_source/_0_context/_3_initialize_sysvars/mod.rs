use super::*;

pub(super) fn initialize_sysvars(
    input: &HistoricalRequest,
    history: &mut History<'_>,
    parent: &[Value],
    slot: u64,
    block: &Value,
    reward_proofs: &mut Vec<Value>,
) -> Result<(Value, Value, Vec<Value>, Value, Roles), Error> {
    let restart_source = history
        .read(&json!({"kind":"account","genesisHash":input.genesis_hash,
        "slot":slot,"pubkey":RESTART,"phase":"end-slot"}))?;
    let mut sysvar_roles = Roles::default();
    sysvar_roles.sysvars.insert(RESTART.into());
    let restart_account = classify_account(
        restart_source["value"].clone(),
        RESTART,
        slot,
        &sysvar_roles,
    )?;
    let restart = restart::prepare_last_restart_from_observation(
        &restart_account,
        &strings(&restart_source["evidenceHashes"])?,
    )?;
    let mut target_sysvars = Vec::new();
    for before in parent {
        let key = before["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("parent account key"))?;
        if !sysvars::BANK_INITIALIZED_SYSVARS.contains(&key) {
            continue;
        }
        sysvar_roles.sysvars.insert(key.into());
        target_sysvars.push(if key == RESTART {
            restart_account.clone()
        } else {
            let account = history.account(key, slot, &sysvar_roles)?;
            super::_4_reward_consistency::repair(input, history, account, block, reward_proofs)?
        });
    }
    let bank_initialized = sysvars::bind_bank_initialized_sysvars(&parent, &target_sysvars, slot)?;
    let accounts = array(&bank_initialized["accounts"], "initialized accounts")?.clone();
    sysvar_roles.sysvars.insert(CLOCK.into());
    let clock = history.account(CLOCK, slot, &sysvar_roles)?;
    sysvars::validate_target_clock(&clock, slot)?;
    Ok((restart, bank_initialized, accounts, clock, sysvar_roles))
}
