use super::*;

/// Original selected transactions plus the original target, not replacement bytes.
pub fn analyze_instructions_sysvar(
    raw: &[Value],
    selected_indices: &[u64],
    target_index: u64,
) -> Result<Value, Error> {
    let pubkey = super::super::history::INSTRUCTIONS;
    let indices = selected_indices
        .iter()
        .copied()
        .chain(std::iter::once(target_index))
        .collect::<Vec<_>>();
    if indices
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != indices.len()
    {
        return Err(bad("execution transaction indices overlap"));
    }
    let mut declared = Vec::new();
    let mut use_count = 0usize;
    let mut target_uses = 0usize;
    for tx in indices {
        let transaction = raw
            .get(tx as usize)
            .ok_or_else(|| bad(format!("transaction {tx} missing")))?;
        let summary = super::super::dependencies::summarize_semantic_transaction(transaction, tx)?;
        if !summary.declared_accounts.iter().any(|id| id == pubkey) {
            continue;
        }
        declared.push(tx);
        let uses = resolve(transaction)?
            .iter()
            .filter(|instruction| instruction.accounts.iter().any(|id| id == pubkey))
            .count();
        if uses == 0 {
            return Err(bad(format!(
                "transaction {tx}: declared Instructions account is unused"
            )));
        }
        use_count = use_count
            .checked_add(uses)
            .ok_or_else(|| bad("Instructions use count overflow"))?;
        if tx == target_index {
            target_uses = uses;
        }
    }
    Ok(
        json!({"pubkey":pubkey,"requirement":"per-transaction-message-account-v1",
        "declaredTransactionIndices":declared,"useCount":use_count,"targetUses":target_uses,
        "runtimeSynthesized":true,"exactAccountRequired":false}),
    )
}
