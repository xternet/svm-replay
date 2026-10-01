use super::*;

pub(super) fn nonmutating_accesses(transactions: &[Value], id: &str) -> Result<Vec<Value>> {
    let mut signatures = BTreeSet::new();
    let mut accesses = Vec::new();
    for (index, transaction) in transactions.iter().enumerate() {
        let version = field(transaction, "version")?;
        check(
            version == "legacy" || version == &json!(0),
            "unreviewed transaction version",
        )?;
        let message = field(field(transaction, "transaction")?, "message")?;
        let meta = field(transaction, "meta")?;
        if version == &json!(0) {
            let lookups = array(field(message, "addressTableLookups")?)?;
            let loaded = field(meta, "loadedAddresses")?;
            for (indices, addresses) in [
                ("writableIndexes", "writable"),
                ("readonlyIndexes", "readonly"),
            ] {
                let mut expected = 0usize;
                for lookup in lookups {
                    expected = expected
                        .checked_add(array(field(lookup, indices)?)?.len())
                        .ok_or_else(|| fail("lookup count overflow"))?;
                }
                check(
                    array(field(loaded, addresses)?)?.len() == expected,
                    "incomplete lookup resolution",
                )?;
            }
        }
        let semantic = summarize_semantic_transaction(transaction, index as u64)?;
        check(
            signatures.insert(semantic.signature),
            "duplicate transaction identity",
        )?;
        check(
            !semantic.address_table_accounts.iter().any(|v| v == id),
            &format!("transaction access:{index}:lookup-table identity"),
        )?;
        if semantic.declared_accounts.iter().any(|v| v == id) {
            let readonly = !semantic.writable_accounts.iter().any(|v| v == id)
                && !semantic.program_ids.iter().any(|v| v == id);
            let persistent = semantic
                .possible_persistent_writes
                .as_ref()
                .is_some_and(|values| values.iter().any(|v| v == id));
            let failed = executed_failure(
                field(meta, "err")?,
                array(field(message, "instructions")?)?.len(),
            ) && semantic.fee_payer != id
                && !persistent;
            check(
                (readonly || failed) && semantic.fee_payer != id && !persistent,
                &format!("transaction access:{index}:possible persistent write"),
            )?;
            accesses.push(
                json!({"index":index,"kind":if readonly{"readonly"}else{"failed-nonpersistent"}}),
            );
        }
    }
    Ok(accesses)
}
