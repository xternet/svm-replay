use super::*;

pub(in super::super) fn compute_closure(
    transactions: &[SemanticTransaction],
    target_index: u64,
    additional: &[String],
    iterations: usize,
) -> Result<SemanticClosureResult> {
    safe_integer(target_index, "target index")?;
    let indexed = index_transactions(transactions)?;
    let target = indexed
        .get(&target_index)
        .ok_or_else(|| invalid(format!("target transaction {target_index} missing")))?;
    let target_position = transactions
        .iter()
        .position(|transaction| transaction.index == target_index)
        .ok_or_else(|| invalid(format!("target transaction {target_index} missing")))?;
    for dependency in additional {
        pubkey(dependency, "additional dependency")?;
    }
    let seed_accounts = unique_sorted(
        target
            .semantic_dependency_accounts
            .iter()
            .chain(&target.program_data_accounts)
            .chain(additional)
            .cloned(),
    );
    let mut application_dependencies: BTreeSet<_> = seed_accounts.iter().cloned().collect();
    let mut selected_reverse = Vec::new();
    let mut intersections_reverse = Vec::new();
    for transaction in transactions[..target_position].iter().rev() {
        // Failed application writes roll back; first-nonce candidates still require replay.
        let writes = if transaction.succeeded {
            transaction.application_writable_accounts.as_slice()
        } else {
            match &transaction.possible_persistent_writes {
                Some(writes) => writes.as_slice(),
                None => &[],
            }
        };
        let intersections = unique(
            writes
                .iter()
                .filter(|key| application_dependencies.contains(*key))
                .cloned(),
        );
        if intersections.is_empty() {
            continue;
        }
        selected_reverse.push(transaction.index);
        intersections_reverse.push(WritableIntersection {
            index: transaction.index,
            accounts: intersections,
        });
        application_dependencies.extend(
            transaction
                .semantic_dependency_accounts
                .iter()
                .chain(&transaction.program_data_accounts)
                .cloned(),
        );
    }
    selected_reverse.reverse();
    intersections_reverse.reverse();
    let mut included = vec![*target];
    for index in &selected_reverse {
        included.push(
            *indexed
                .get(index)
                .ok_or_else(|| invalid(format!("selected transaction {index} missing")))?,
        );
    }
    let program_data_accounts = unique_sorted(
        additional
            .iter()
            .chain(
                included
                    .iter()
                    .flat_map(|transaction| &transaction.program_data_accounts),
            )
            .cloned(),
    );
    let dependency_accounts = unique_sorted(
        included
            .iter()
            .flat_map(|transaction| &transaction.declared_accounts)
            .chain(
                included
                    .iter()
                    .flat_map(|transaction| &transaction.address_table_accounts),
            )
            .chain(
                included
                    .iter()
                    .flat_map(|transaction| &transaction.program_ids),
            )
            .chain(&program_data_accounts)
            .cloned(),
    );
    let excluded_dependency_accounts = unique_sorted(
        included
            .iter()
            .flat_map(|transaction| &transaction.excluded_dependency_accounts)
            .chain(
                dependency_accounts
                    .iter()
                    .filter(|key| EXCLUDED.contains(&key.as_str())),
            )
            .cloned(),
    );
    let included_vote_indices = selected_reverse
        .iter()
        .filter(|index| {
            indexed
                .get(index)
                .is_some_and(|transaction| transaction.is_vote)
        })
        .copied()
        .collect();
    Ok(SemanticClosureResult {
        seed_accounts,
        selected_indices: selected_reverse,
        application_dependency_accounts: unique_sorted(application_dependencies),
        dependency_accounts,
        program_data_accounts,
        excluded_dependency_accounts,
        fee_only_payers_ignored: unique_sorted(
            transactions[..=target_position]
                .iter()
                .filter(|transaction| transaction.fee_only_payer)
                .map(|transaction| transaction.fee_payer.clone()),
        ),
        included_vote_indices,
        writable_intersections: intersections_reverse,
        iterations,
    })
}
