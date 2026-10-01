use super::*;

pub(in super::super) fn index_transactions(
    transactions: &[SemanticTransaction],
) -> Result<BTreeMap<u64, &SemanticTransaction>> {
    let mut indexed = BTreeMap::new();
    let mut previous = None;
    for transaction in transactions {
        safe_integer(transaction.index, "transaction index")?;
        if previous.is_some_and(|index| transaction.index <= index) {
            return Err(invalid("transactions are not in strict ledger order"));
        }
        indexed.insert(transaction.index, transaction);
        previous = Some(transaction.index);
    }
    Ok(indexed)
}

pub fn compute_backward_semantic_closure(
    transactions: &[SemanticTransaction],
    target_index: u64,
    additional_dependencies: &[String],
) -> Result<SemanticClosureResult> {
    compute_closure(transactions, target_index, additional_dependencies, 1)
}

/// Caller-resolved ProgramData discovery, bounded exactly as the original finite algorithm.
pub fn compute_program_data_fixed_point<F>(
    transactions: &[SemanticTransaction],
    target_index: u64,
    mut resolve: F,
) -> Result<SemanticClosureResult>
where
    F: FnMut(&[String]) -> Result<BTreeMap<String, Option<String>>>,
{
    let indexed = index_transactions(transactions)?;
    if !indexed.contains_key(&target_index) {
        return Err(invalid(format!(
            "target transaction {target_index} missing"
        )));
    }
    let mut program_data = BTreeSet::new();
    let mut inspected = BTreeSet::new();
    for iteration in 1..=transactions.len() + 2 {
        let mut closure = compute_closure(
            transactions,
            target_index,
            &program_data.iter().cloned().collect::<Vec<_>>(),
            iteration,
        )?;
        let uninspected: Vec<_> = closure
            .dependency_accounts
            .iter()
            .filter(|key| !inspected.contains(*key))
            .cloned()
            .collect();
        if uninspected.is_empty() {
            closure.program_data_accounts = unique_sorted(program_data);
            return Ok(closure);
        }
        let resolved = resolve(&uninspected)?;
        for key in uninspected {
            let address = resolved
                .get(&key)
                .ok_or_else(|| invalid(format!("ProgramData resolver omitted {key}")))?;
            if let Some(address) = address {
                pubkey(address, &format!("ProgramData address for {key}"))?;
                program_data.insert(address.clone());
            }
            inspected.insert(key);
        }
    }
    Err(invalid(format!(
        "transaction {target_index}: ProgramData closure did not converge"
    )))
}
