use super::*;

pub(super) const SYSTEM: &str = "11111111111111111111111111111111";

pub(super) const RECENT: &str = "SysvarRecentB1ockHashes11111111111111111111";

pub(super) fn raw(
    signature: &str,
    keys: &[&str],
    writable_unsigned: usize,
    instructions: Value,
) -> Value {
    json!({"transaction":{"signatures":[signature],"message":{"header":{"numRequiredSignatures":1,"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":keys.len()-1-writable_unsigned},"accountKeys":keys,"instructions":instructions}},
        "meta":{"err":null,"loadedAddresses":{"writable":[],"readonly":[]},"innerInstructions":[]}})
}

pub(super) fn simple(signature: &str, payer: &str, state: &str, program: &str) -> Value {
    raw(
        signature,
        &[payer, state, program],
        1,
        json!([{"programIdIndex":2,"accounts":[1],"data":""}]),
    )
}

pub(super) fn summary(raw: &Value, index: u64) -> SemanticTransaction {
    summarize_semantic_transaction(raw, index).expect("valid test transaction")
}

pub(super) fn semantic(
    index: u64,
    writes: &[&str],
    reads: &[&str],
    program: &str,
) -> SemanticTransaction {
    let mut tx = summary(
        &simple(
            &format!("signature-{index}"),
            &format!("payer-{index}"),
            "unused",
            program,
        ),
        index,
    );
    tx.declared_accounts = std::iter::once(tx.fee_payer.clone())
        .chain(reads.iter().map(|s| s.to_string()))
        .collect();
    tx.instruction_accounts = reads.iter().map(|s| s.to_string()).collect();
    tx.writable_accounts = writes.iter().map(|s| s.to_string()).collect();
    tx.application_writable_accounts = tx.writable_accounts.clone();
    tx.semantic_dependency_accounts = reads.iter().map(|s| s.to_string()).collect();
    tx
}

pub(super) fn nonce(first: bool, trailing: bool) -> Value {
    let data = if trailing {
        vec![4, 0, 0, 0, 99]
    } else {
        vec![4, 0, 0, 0]
    };
    let advance =
        json!({"programIdIndex":4,"accounts":[1,2,0],"data":bs58::encode(data).into_string()});
    let other = json!({"programIdIndex":4,"accounts":[0,3],"data":""});
    let mut value = raw(
        "failed-nonce",
        &["payer", "nonce", RECENT, "recipient", SYSTEM],
        3,
        if first {
            json!([advance, other])
        } else {
            json!([other, advance])
        },
    );
    value["meta"]["err"] = json!({"InstructionError":[1,"InvalidArgument"]});
    value
}

pub(super) fn reference_paths(
    transactions: &[SemanticTransaction],
    position: usize,
    needs: &BTreeSet<String>,
    selected: &mut BTreeSet<u64>,
) {
    for previous in 0..position {
        let transaction = &transactions[previous];
        let writes = if transaction.succeeded {
            transaction.application_writable_accounts.as_slice()
        } else {
            match &transaction.possible_persistent_writes {
                Some(writes) => writes.as_slice(),
                None => &[],
            }
        };
        if writes.iter().any(|key| needs.contains(key)) {
            selected.insert(transaction.index);
            let dependencies = transaction
                .semantic_dependency_accounts
                .iter()
                .chain(&transaction.program_data_accounts)
                .cloned()
                .collect();
            reference_paths(transactions, previous, &dependencies, selected);
        }
    }
}
