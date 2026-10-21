use super::*;

pub(in super::super) struct Images {
    pub accounts: Vec<Value>,
    pub clock: Value,
    pub initialized: Value,
    pub restart: Value,
    pub migrations: Value,
}

pub(in super::super) fn acquire(
    input: &InitializationRequest,
    block: &Value,
    history: &mut History<'_>,
) -> Result<Images, Error> {
    let parent = position(block, input.after_slot, input.slot)?;
    let mut roles = Roles::default();
    roles
        .sysvars
        .extend(sysvars::GENERIC_SYSVARS.iter().map(|id| (*id).to_owned()));
    let mut keys = BTreeSet::from([
        "SysvarRent111111111111111111111111111111111".to_owned(),
        "SysvarLastRestartS1ot1111111111111111111111".to_owned(),
    ]);
    keys.extend(
        input
            .extra_accounts
            .iter()
            .filter(|id| id.as_str() != CLOCK)
            .cloned(),
    );
    let mut accounts = Vec::new();
    loop {
        let missing = keys
            .iter()
            .filter(|id| !accounts.iter().any(|a: &Value| a["pubkey"] == **id))
            .cloned()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            break;
        }
        for key in missing {
            let mut account = history.account(&key, parent, &roles)?;
            if let Some(link) = history::program_data_address(&account)? {
                roles.program_data.insert(link.clone());
                keys.insert(link);
            }
            if account["presence"] == "present" && account["executable"] == true {
                account["role"] = json!("program");
                let owner = account["owner"]
                    .as_str()
                    .ok_or_else(|| invalid("program owner missing"))?;
                if [
                    "BPFLoader1111111111111111111111111111111111",
                    "BPFLoader2111111111111111111111111111111111",
                    history::UPGRADEABLE_LOADER,
                    "LoaderV411111111111111111111111111111111111",
                ]
                .contains(&owner)
                {
                    keys.insert(owner.to_owned());
                }
            }
            accounts.push(account);
        }
        // Discover ProgramData and implicit-loader images before inspecting an
        // executable; an unresolved frontier is not a missing program binary.
        if keys
            .iter()
            .any(|id| !accounts.iter().any(|a| a["pubkey"] == *id))
        {
            continue;
        }
        let programs = accounts
            .iter()
            .filter(|a| a["executable"] == true)
            .map(|a| {
                a["pubkey"]
                    .as_str()
                    .ok_or_else(|| invalid("program address missing"))
                    .map(str::to_owned)
            })
            .collect::<Result<Vec<_>, _>>()?;
        keys.extend(imports::dedicated_inputs(&programs, &accounts)?);
    }
    for account in &mut accounts {
        let key = account["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("account address missing"))?
            .to_owned();
        if roles.program_data.contains(&key) {
            account["role"] = json!("programdata");
        }
    }
    // As for transaction boundaries: an archive null may be excluded data (vote
    // accounts), so the block itself must show the account empty where it first appears.
    let transactions = block["result"]["transactions"]
        .as_array()
        .ok_or_else(|| invalid("block transactions missing"))?;
    history::assert_account_coverage(&accounts, transactions, parent)?;
    let migrations = migration::program_migration_context(
        &json!({"slot":input.slot,"parentSlot":parent,
        "blockSourceHash":input.block_sha256,"envelope":block,"runtime":input.runtime_binding,"requestedPubkeys":keys}),
    )?;
    migration::validate_program_migration_parents(&migrations, &accounts)?;
    let mut target = Vec::new();
    for account in &accounts {
        let id = account["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("account address missing"))?;
        if sysvars::BANK_INITIALIZED_SYSVARS.contains(&id) {
            target.push(history.account(id, input.slot, &roles)?);
        }
    }
    let initialized = sysvars::bind_bank_initialized_sysvars(&accounts, &target, input.slot)?;
    let accounts = initialized["accounts"]
        .as_array()
        .ok_or_else(|| invalid("initialized images missing"))?
        .clone();
    let clock = history.account(CLOCK, input.slot, &roles)?;
    sysvars::validate_target_clock(&clock, input.slot)?;
    let restart = restart::prepare_last_restart_from_observation(
        target
            .iter()
            .find(|a| a["pubkey"] == "SysvarLastRestartS1ot1111111111111111111111")
            .ok_or_else(|| invalid("initialized restart image missing"))?,
        &history
            .sources
            .observations()
            .iter()
            .filter_map(|o| o["evidenceHashes"].as_array())
            .flatten()
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| invalid("evidence hash missing"))
                    .map(str::to_owned)
            })
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    Ok(Images {
        accounts,
        clock,
        initialized,
        restart,
        migrations,
    })
}
