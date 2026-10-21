use super::*;

pub(in super::super) const RECENT: &str = "SysvarRecentB1ockHashes11111111111111111111";

pub(in super::super) struct Instruction {
    pub(in super::super) program: String,
    pub(in super::super) accounts: Vec<String>,
    pub(in super::super) data: Vec<u8>,
    pub(in super::super) inner: bool,
}

pub(in super::super) fn resolve(transaction: &Value) -> Result<Vec<Instruction>> {
    let message = field(field(transaction, "transaction")?, "message")?;
    let meta = field(transaction, "meta")?;
    let loaded = field(meta, "loadedAddresses")?;
    let mut keys = Vec::new();
    for list in [
        field(message, "accountKeys")?,
        field(loaded, "writable")?,
        field(loaded, "readonly")?,
    ] {
        for value in array(list)? {
            let key = string(value)?;
            check(!key.is_empty(), "empty account key")?;
            keys.push(key.to_owned());
        }
    }
    let read = |values: &Value, inner: bool| -> Result<Vec<Instruction>> {
        array(values)?
            .iter()
            .map(|value| {
                let program = if let Some(Value::String(program)) = value.get("programId") {
                    program.clone()
                } else {
                    keys.get(integer(field(value, "programIdIndex")?)? as usize)
                        .cloned()
                        .ok_or_else(|| fail("program index out of range"))?
                };
                let mut accounts = Vec::new();
                for account in array(field(value, "accounts")?)? {
                    if let Some(key) = account.as_str() {
                        check(
                            keys.iter().any(|value| value == key),
                            "undeclared instruction account",
                        )?;
                        accounts.push(key.into());
                    } else {
                        accounts.push(
                            keys.get(integer(account)? as usize)
                                .cloned()
                                .ok_or_else(|| fail("account index out of range"))?,
                        );
                    }
                }
                let encoded = string(field(value, "data")?)?;
                let data = bs58::decode(encoded)
                    .into_vec()
                    .map_err(|e| fail(format!("instruction data: {e}")))?;
                Ok(Instruction {
                    program,
                    accounts,
                    data,
                    inner,
                })
            })
            .collect()
    };
    let mut instructions = read(field(message, "instructions")?, false)?;
    match meta.get("innerInstructions") {
        None | Some(Value::Null) => {}
        Some(groups) => {
            for group in array(groups)? {
                integer(field(group, "index")?)?;
                instructions.extend(read(field(group, "instructions")?, true)?);
            }
        }
    }
    Ok(instructions)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct FeeWitness {
    pub(in super::super) transaction_index: usize,
    pub(in super::super) fee: u64,
    pub(in super::super) signatures: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Lineage<'a> {
    pub(in super::super) source_hash: &'a str,
    pub(in super::super) target_slot: u64,
    pub(in super::super) parent_slot: u64,
    pub(in super::super) blockhash: &'a str,
    pub(in super::super) previous_blockhash: &'a str,
}

/// Shared metadata witnesses; never executes any archived transaction.
pub(in super::super) fn derive_context(
    block: &Value,
    target_slot: u64,
    source_hash: &str,
    required: Value,
) -> Result<Value> {
    digest(source_hash)?;
    integer(&json!(target_slot))?;
    let parent = integer(field(block, "parentSlot")?)?;
    let blockhash = pubkey(field(block, "blockhash")?)?;
    let execution = pubkey(field(block, "previousBlockhash")?)?;
    check(parent < target_slot, "parent must precede target")?;
    let transactions = array(field(block, "transactions")?)?;
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
    let mut result = json!({"targetSlot":target_slot,"parentSlot":parent,"executionBlockhash":execution,"lamportsPerSignature":rate,
        "sourceKind":"canonical-block-lineage-and-vote-fees-v1","blockSourceHash":source_hash,"blockLineageHash":lineage_hash,
        "feeWitnessHash":witness_hash,"feeWitnessCount":witnesses.len(),"feeWitnessIndicesHash":indices_hash,"requiredRuntimeSysvars":required});
    result["derivationHash"] = json!(historical_bank_context_hash(&result)?);
    Ok(result)
}
