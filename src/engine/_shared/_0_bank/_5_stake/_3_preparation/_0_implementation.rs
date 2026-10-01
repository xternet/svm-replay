use super::*;

pub fn recover_untouched_reward_stake(input: &Value) -> Result<Value> {
    recover(input).map_err(|error| Error::new("UNSUPPORTED_REWARD_RECOVERY", error.to_string()))
}

pub fn prepare_stake_initialization(input: &Value) -> Result<Value> {
    let accounts = array(field(input, "accounts")?)?;
    let stakes: Vec<_> = accounts
        .iter()
        .filter(|account| {
            account.get("presence") == Some(&json!("present"))
                && account.get("owner") == Some(&json!(STAKE))
        })
        .collect();
    if stakes.is_empty() {
        check(
            input.get("evidence").is_none(),
            "unrelated stake initialization evidence",
        )?;
        return Ok(json!({"accounts":accounts,"proofs":[]}));
    }
    if let Some(evidence) = input.get("inactiveRewards") {
        check(
            input.get("evidence").is_none(),
            "conflicting stake initialization proofs",
        )?;
        return initialize_inactive(input, evidence);
    }
    let unsupported = |detail: &str| {
        Error::new(
            "UNSUPPORTED_BANK_INPUT",
            format!(
                "unproven pre-transaction stake initialization; implementation limitation:{detail}"
            ),
        )
    };
    let evidence = input
        .get("evidence")
        .ok_or_else(|| unsupported("required stake evidence missing"))?;
    let evidence_accounts = array(field(evidence, "accounts")?)?;
    let mut seen = BTreeSet::new();
    check(
        evidence_accounts.len() == stakes.len(),
        "missing/duplicate/unrelated stake evidence",
    )
    .map_err(|_| unsupported("evidence scope differs"))?;
    for entry in evidence_accounts {
        let id = string(field(entry, "pubkey")?)?;
        if !seen.insert(id)
            || !stakes
                .iter()
                .any(|stake| stake.get("pubkey") == Some(&json!(id)))
        {
            return Err(unsupported(
                "missing, duplicate or unrelated stake evidence",
            ));
        }
    }
    let block_source = field(evidence, "block")?;
    let raw = bytes(field(block_source, "responseBodyBase64")?)?;
    let source_hash = field(input, "blockSourceHash")?;
    let envelope = field(input, "envelope")?;
    check(
        source_hash == &json!(hash(&raw))
            && field(block_source, "responseSha256")? == source_hash
            && canonical_json(&svm_replay_protocol::parse_json(&raw)?) == canonical_json(envelope),
        "stake initialization block source differs from prepared canonical block",
    )?;
    let block = field(envelope, "result")?;
    let mut recovered = BTreeMap::new();
    let mut proofs = Vec::new();
    for stake in stakes {
        let id = string(field(stake, "pubkey")?)?;
        let source = evidence_accounts
            .iter()
            .find(|entry| entry.get("pubkey") == Some(&json!(id)))
            .ok_or_else(|| unsupported("stake source missing"))?;
        let mut request = json!({"pubkey":id,"boundary":{"slot":field(input,"slot")?,"parentSlot":field(input,"parentSlot")?,
            "blockhash":field(block,"blockhash")?,"blockHeight":field(block,"blockHeight")?,"transactionCount":array(field(block,"transactions")?)?.len()},
            "runtime":field(input,"runtime")?,"block":block_source,"parentBlock":field(evidence,"parentBlock")?,
            "parentAccount":field(source,"parentAccount")?,"targetAccount":field(source,"targetAccount")?});
        if let Some(context) = input.get("phaseContext") {
            request["phaseContext"] = context.clone();
        }
        let result = recover_untouched_reward_stake(&request)?;
        let mut parent = field(&result, "parentAccount")?.clone();
        parent["role"] = field(stake, "role")?.clone();
        check(
            canonical_json(&parent) == canonical_json(stake),
            &format!("stake initialization parent image differs from prepared input:{id}"),
        )?;
        let mut account = field(&result, "account")?.clone();
        account["role"] = field(stake, "role")?.clone();
        let mut proof = field(&result, "proof")?.clone();
        proof["initializedAccountSha256"] = json!(hash(canonical_json(&account)));
        recovered.insert(id, account);
        proofs.push(proof);
    }
    let accounts: Vec<_> = accounts
        .iter()
        .map(|account| {
            let id = string(field(account, "pubkey")?)?;
            Ok(match recovered.get(id) {
                Some(account) => account.clone(),
                None => account.clone(),
            })
        })
        .collect::<Result<_>>()?;
    Ok(json!({"accounts":accounts,"proofs":proofs}))
}
