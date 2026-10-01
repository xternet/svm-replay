use super::*;

// Bank::freeze distributes transaction fees after all transactions. The archive
// records that frozen balance; the worker stops at the transaction boundary.
pub(in super::super::super) fn end_transaction_accounts(
    fixture: &Value,
    block: &Value,
    target_slot: u64,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut accounts = array(field(fixture, "endAccounts")?)?.clone();
    let mut keys = BTreeSet::new();
    for account in &accounts {
        check(
            keys.insert(string(field(account, "pubkey")?)?),
            "duplicate end account",
        )?;
        equal(
            field(account, "sourceSlot")?,
            &json!(target_slot),
            "end account source slot",
        )?;
    }
    let mut fees = BTreeMap::new();
    if let Some(rewards) = block.get("rewards") {
        for reward in array(rewards)? {
            if reward.get("rewardType") != Some(&json!("Fee")) {
                continue;
            }
            let key = string(field(reward, "pubkey")?)?;
            check(fees.insert(key, reward).is_none(), "duplicate fee reward")?;
        }
    }
    let mut proofs = Vec::new();
    for account in &mut accounts {
        let key = string(field(account, "pubkey")?)?.to_owned();
        let Some(reward) = fees.get(key.as_str()) else {
            continue;
        };
        check(
            field(account, "presence")? == "present",
            "fee recipient missing from archive",
        )?;
        let frozen = unsigned(field(account, "lamports")?)?;
        let amount = unsigned(field(reward, "lamports")?)?;
        check(
            unsigned(field(reward, "postBalance")?)? == frozen,
            "fee reward post-balance mismatch",
        )?;
        let before = frozen
            .checked_sub(amount)
            .ok_or_else(|| VerificationError("fee reward exceeds frozen balance".into()))?;
        account["lamports"] = json!(before.to_string());
        proofs.push(json!({"pubkey":key,"lamports":amount.to_string(),
            "frozenLamports":frozen.to_string(),"transactionLamports":before.to_string(),
            "basis":"block-fee-reward-at-freeze/v1"}));
    }
    Ok((accounts, proofs))
}
