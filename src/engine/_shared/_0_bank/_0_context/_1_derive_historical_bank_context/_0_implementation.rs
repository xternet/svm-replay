use super::*;

/// The caller supplies the semantic predecessor closure; this function performs no acquisition.
pub fn derive_historical_bank_context(
    envelope: &Value,
    selector: &Value,
    source_hash: &str,
    selected_indices: &[usize],
) -> Result<Value> {
    digest(source_hash)?;
    let block = field(envelope, "result")?;
    let parent = integer(field(block, "parentSlot")?)?;
    let blockhash = pubkey(field(block, "blockhash")?)?;
    let execution = pubkey(field(block, "previousBlockhash")?)?;
    let target_slot = integer(field(selector, "slot")?)?;
    check(parent < target_slot, "parent must precede target")?;
    check(
        field(selector, "parentSlot")? == &json!(parent)
            && field(selector, "blockhash")? == blockhash,
        "historical Bank selector mismatch",
    )?;
    let target_index = integer(field(selector, "index")?)? as usize;
    let transactions = array(field(block, "transactions")?)?;
    let target = transactions
        .get(target_index)
        .ok_or_else(|| fail("historical Bank target missing"))?;
    let signatures = array(field(field(target, "transaction")?, "signatures")?)?;
    check(
        signatures.first() == Some(field(selector, "signature")?),
        "historical Bank target signature mismatch",
    )?;
    let mut seen = BTreeSet::new();
    let mut nonce_uses = 0usize;
    let mut runtime_resolvable = true;
    for index in selected_indices
        .iter()
        .copied()
        .chain(std::iter::once(target_index))
    {
        check(
            index <= target_index && seen.insert(index),
            "execution transaction indices overlap or follow target",
        )?;
        for instruction in resolve(
            transactions
                .get(index)
                .ok_or_else(|| fail("selected transaction missing"))?,
        )? {
            for (position, id) in instruction.accounts.iter().enumerate() {
                if id == RECENT {
                    nonce_uses += 1;
                    runtime_resolvable &= !instruction.inner
                        && instruction.program == "11111111111111111111111111111111"
                        && position == 1
                        && instruction.data.starts_with(&[4, 0, 0, 0]);
                }
            }
        }
    }
    let mut witnesses = Vec::new();
    let mut per_signature = None;
    for (index, transaction) in transactions.iter().enumerate() {
        let meta = field(transaction, "meta")?;
        if !field(meta, "err")?.is_null() {
            continue;
        }
        let instructions = resolve(transaction)?;
        if instructions.len() != 1
            || instructions[0].program != "Vote111111111111111111111111111111111111111"
        {
            continue;
        }
        let header = field(
            field(field(transaction, "transaction")?, "message")?,
            "header",
        )?;
        let signatures = integer(field(header, "numRequiredSignatures")?)?;
        let fee = integer(field(meta, "fee")?)?;
        check(
            signatures > 0 && fee > 0 && fee % signatures == 0,
            &format!("vote fee witness {index} malformed"),
        )?;
        let rate = fee / signatures;
        if let Some(previous) = per_signature {
            check(previous == rate, "vote fee witnesses disagree")?;
        } else {
            per_signature = Some(rate);
        }
        witnesses.push(FeeWitness {
            transaction_index: index,
            fee,
            signatures,
        });
    }
    let rate = per_signature.ok_or_else(|| fail("vote fee witnesses missing"))?;
    let lineage = Lineage {
        source_hash,
        target_slot,
        parent_slot: parent,
        blockhash,
        previous_blockhash: execution,
    };
    let lineage_hash = hash(serde_json::to_vec(&lineage).map_err(|e| fail(e.to_string()))?);
    let witness_hash = hash(serde_json::to_vec(&witnesses).map_err(|e| fail(e.to_string()))?);
    let witness_indices: Vec<_> = witnesses.iter().map(|w| w.transaction_index).collect();
    let indices_hash = hash(serde_json::to_vec(&witness_indices).map_err(|e| fail(e.to_string()))?);
    let required = if nonce_uses > 0 && runtime_resolvable {
        json!([{"pubkey":RECENT,"requirement":"nonce-advance-nonempty-cache-v1"}])
    } else {
        json!([])
    };
    let mut result = json!({"targetSlot":target_slot,"parentSlot":parent,"executionBlockhash":execution,"lamportsPerSignature":rate,
        "sourceKind":"canonical-block-lineage-and-vote-fees-v1","blockSourceHash":source_hash,"blockLineageHash":lineage_hash,
        "feeWitnessHash":witness_hash,"feeWitnessCount":witnesses.len(),"feeWitnessIndicesHash":indices_hash,"requiredRuntimeSysvars":required});
    result["derivationHash"] = json!(historical_bank_context_hash(&result)?);
    Ok(result)
}

/// Bind a target block hash to the existing pinned lineage, not the parent's
/// execution block hash used while executing transactions inside that block.
pub fn assert_target_blockhash(context: &Value, blockhash: &str) -> Result<()> {
    pubkey(&json!(blockhash))?;
    let lineage = Lineage {
        source_hash: string(field(context, "blockSourceHash")?)?,
        target_slot: integer(field(context, "targetSlot")?)?,
        parent_slot: integer(field(context, "parentSlot")?)?,
        blockhash,
        previous_blockhash: string(field(context, "executionBlockhash")?)?,
    };
    check(
        field(context, "blockLineageHash")?
            == &json!(hash(
                serde_json::to_vec(&lineage).map_err(|e| fail(e.to_string()))?
            )),
        "target block hash does not match pinned lineage",
    )
}

pub fn historical_bank_context_hash(context: &Value) -> Result<String> {
    let mut lines = vec!["svm-call-historical-bank-context-v1".to_owned()];
    for key in [
        "targetSlot",
        "parentSlot",
        "executionBlockhash",
        "lamportsPerSignature",
        "sourceKind",
        "blockSourceHash",
        "blockLineageHash",
        "feeWitnessCount",
        "feeWitnessIndicesHash",
        "feeWitnessHash",
    ] {
        let value = field(context, key)?;
        lines.push(
            if [
                "targetSlot",
                "parentSlot",
                "lamportsPerSignature",
                "feeWitnessCount",
            ]
            .contains(&key)
            {
                integer(value)?.to_string()
            } else {
                string(value)?.to_owned()
            },
        );
    }
    for entry in array(field(context, "requiredRuntimeSysvars")?)? {
        lines.push(string(field(entry, "pubkey")?)?.into());
        lines.push(string(field(entry, "requirement")?)?.into());
    }
    Ok(hash(format!("{}\n", lines.join("\n"))))
}
