use super::*;

/// Summarizes captured metadata; it does not validate wire admission or signature authenticity.
pub fn summarize_semantic_transaction(input: &Value, index: u64) -> Result<SemanticTransaction> {
    safe_integer(index, "transaction index")?;
    object(input, &format!("transaction {index}"))?;
    let transaction = &input["transaction"];
    object(transaction, &format!("transaction {index} transaction"))?;
    let message = &transaction["message"];
    let message_object = object(message, &format!("transaction {index} message"))?;
    let header = header(&message["header"], index)?;
    let static_accounts = read_pubkeys(
        &message["accountKeys"],
        &format!("transaction {index} accountKeys"),
    )?;
    if header.required == 0 || static_accounts.is_empty() {
        return Err(invalid(format!("transaction {index}: fee payer missing")));
    }
    let meta = &input["meta"];
    let meta_object = object(meta, &format!("transaction {index} meta"))?;
    if !meta_object.contains_key("err") {
        return Err(invalid(format!(
            "transaction {index}: meta error field missing"
        )));
    }
    let (loaded_writable, loaded_readonly) = loaded(meta_object.get("loadedAddresses"), index)?;
    let resolved: Vec<_> = static_accounts
        .iter()
        .chain(&loaded_writable)
        .chain(&loaded_readonly)
        .cloned()
        .collect();
    let declared_accounts = unique(resolved.clone());
    let outer = instructions(&message["instructions"], &resolved, index, "instruction")?;
    let inner = inner_instructions(meta_object.get("innerInstructions"), &resolved, index)?;
    let instruction_accounts = unique(outer.accounts.iter().chain(&inner.accounts).cloned());
    let program_ids = unique(outer.programs.iter().chain(&inner.programs).cloned());
    let address_table_accounts = address_tables(message_object.get("addressTableLookups"), index)?;
    let requested_writable = unique(
        static_writable(&static_accounts, &header)?
            .into_iter()
            .chain(loaded_writable),
    );
    let demoted: BTreeSet<_> = if declared_accounts
        .iter()
        .any(|key| key == "BPFLoaderUpgradeab1e11111111111111111111111")
    {
        BTreeSet::new()
    } else {
        outer.programs.iter().cloned().collect()
    };
    let writable_accounts: Vec<_> = requested_writable
        .into_iter()
        .filter(|key| !demoted.contains(key))
        .collect();
    let referenced: BTreeSet<_> = instruction_accounts.iter().collect();
    let fee_payer = static_accounts[0].clone();
    let fee_only_payer = !referenced.contains(&fee_payer);
    let readonly_authority = readonly_vote_authority(input, &resolved);
    let semantic_dependency_accounts = unique(
        instruction_accounts
            .iter()
            .chain(&address_table_accounts)
            .chain(&program_ids)
            .cloned(),
    )
    .into_iter()
    .filter(|key| !fee_only_payer || *key != fee_payer)
    .collect::<Vec<_>>();
    let mut possible_persistent_writes = None;
    if !meta["err"].is_null() {
        if let Some(first) = array(
            &message["instructions"],
            &format!("transaction {index} instructions"),
        )?
        .first()
        {
            if outer
                .programs
                .first()
                .is_some_and(|program| program == "11111111111111111111111111111111")
            {
                if let Some(data) = first["data"].as_str() {
                    let data = bs58::decode(data).into_vec().map_err(|e| {
                        invalid(format!(
                            "transaction {index}: nonce instruction base58 malformed: {e}"
                        ))
                    })?;
                    if data.starts_with(&[4, 0, 0, 0]) {
                        if let Some(reference) =
                            array(&first["accounts"], "nonce instruction accounts")?.first()
                        {
                            possible_persistent_writes = Some(vec![resolve_reference(
                                reference,
                                &resolved,
                                "nonce account",
                            )?]);
                        }
                    }
                }
            }
        }
    }
    let signatures = array(
        &transaction["signatures"],
        &format!("transaction {index} signatures"),
    )?;
    let signature = signatures
        .first()
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid(format!("transaction {index}: signature missing")))?
        .to_owned();
    Ok(SemanticTransaction {
        index,
        signature,
        fee_payer,
        declared_accounts,
        application_writable_accounts: writable_accounts
            .iter()
            .filter(|key| referenced.contains(key) && readonly_authority.as_ref() != Some(*key))
            .cloned()
            .collect(),
        instruction_accounts,
        writable_accounts,
        possible_persistent_writes,
        address_table_accounts,
        is_vote: program_ids.iter().any(|id| id == VOTE_PROGRAM),
        program_ids,
        program_data_accounts: Vec::new(),
        excluded_dependency_accounts: semantic_dependency_accounts
            .iter()
            .filter(|key| EXCLUDED.contains(&key.as_str()))
            .cloned()
            .collect(),
        semantic_dependency_accounts,
        fee_only_payer,
        succeeded: meta["err"].is_null(),
    })
}

// Vote state updates write their vote account, not the authority. A fee payer
// can also be that read-only signer. Omitted-fee planning still retains its fee.
fn readonly_vote_authority(input: &Value, keys: &[String]) -> Option<String> {
    let meta = &input["meta"];
    if !meta["err"].is_null()
        || meta["innerInstructions"].as_array()?.iter().any(|group| {
            group["instructions"]
                .as_array()
                .is_none_or(|v| !v.is_empty())
        })
    {
        return None;
    }
    let instructions = input["transaction"]["message"]["instructions"].as_array()?;
    if instructions.len() != 1 {
        return None;
    }
    let instruction = &instructions[0];
    let program = keys.get(usize::try_from(instruction["programIdIndex"].as_u64()?).ok()?)?;
    if program != VOTE_PROGRAM {
        return None;
    }
    let bytes = bs58::decode(instruction["data"].as_str()?)
        .into_vec()
        .ok()?;
    let opcode = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?);
    // VoteInstruction::{UpdateVoteState, UpdateVoteStateSwitch,
    // CompactUpdateVoteState, CompactUpdateVoteStateSwitch, TowerSync, TowerSyncSwitch}.
    if !matches!(opcode, 8 | 9 | 12 | 13 | 14 | 15) {
        return None;
    }
    let accounts = instruction["accounts"].as_array()?;
    if accounts.len() != 2 || accounts[0] == accounts[1] {
        return None;
    }
    let authority = keys.get(usize::try_from(accounts[1].as_u64()?).ok()?)?;
    (Some(authority) == keys.first()).then(|| authority.clone())
}
