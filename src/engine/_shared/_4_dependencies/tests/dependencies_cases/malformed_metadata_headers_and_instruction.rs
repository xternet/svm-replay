use super::*;

#[test]
fn malformed_metadata_headers_and_instruction_references_reject() {
    let base = simple("tx", "payer", "state", "program");
    let mut cases = Vec::new();
    for field in ["transaction", "meta"] {
        let mut value = base.clone();
        value.as_object_mut().expect("object").remove(field);
        cases.push(value);
    }
    let mut value = base.clone();
    value["meta"].as_object_mut().expect("meta").remove("err");
    cases.push(value);
    for (field, n) in [
        ("numRequiredSignatures", 0),
        ("numRequiredSignatures", 4),
        ("numReadonlySignedAccounts", 2),
        ("numReadonlyUnsignedAccounts", 3),
    ] {
        let mut value = base.clone();
        value["transaction"]["message"]["header"][field] = json!(n);
        cases.push(value);
    }
    for accounts in [json!([8]), json!([-1]), json!([1.5]), json!(["undeclared"])] {
        let mut value = base.clone();
        value["transaction"]["message"]["instructions"][0]["accounts"] = accounts;
        cases.push(value);
    }
    let mut value = base.clone();
    value["transaction"]["message"]["instructions"][0]["programIdIndex"] = json!(3);
    cases.push(value);
    let mut value = base.clone();
    value["transaction"]["signatures"] = json!([]);
    cases.push(value);
    let mut value = base.clone();
    value["meta"]["innerInstructions"] = json!([{"index":-1,"instructions":[]}]);
    cases.push(value);
    for value in cases {
        assert!(
            summarize_semantic_transaction(&value, 0).is_err(),
            "{value}"
        );
    }
    assert!(summarize_semantic_transaction(&base, 9_007_199_254_740_992).is_err());
    let mut bad = nonce(true, false);
    bad["transaction"]["message"]["instructions"][0]["data"] = json!("0");
    assert!(summarize_semantic_transaction(&bad, 0).is_err());
}

#[test]
fn absent_optional_rpc_sections_are_specified_empty_sections() {
    let mut raw = simple("legacy", "payer", "state", "program");
    raw["meta"]
        .as_object_mut()
        .expect("meta")
        .remove("loadedAddresses");
    raw["meta"]
        .as_object_mut()
        .expect("meta")
        .remove("innerInstructions");
    let transaction = summary(&raw, 0);
    assert_eq!(
        transaction.declared_accounts,
        vec!["payer", "state", "program"]
    );
    assert!(transaction.address_table_accounts.is_empty());
    assert!(serde_json::to_value(transaction)
        .expect("serialize")
        .get("possiblePersistentWrites")
        .is_none());
}

#[test]
fn closure_rejects_wrong_order_missing_target_and_empty_additional_dependency() {
    let first = summary(&simple("first", "payer", "state", "program"), 0);
    let second = summary(&simple("second", "payer", "state", "program"), 2);
    for transactions in [
        vec![second.clone(), first.clone()],
        vec![first.clone(), first.clone()],
    ] {
        assert!(compute_backward_semantic_closure(&transactions, 0, &[]).is_err());
    }
    assert!(compute_backward_semantic_closure(&[first.clone()], 2, &[]).is_err());
    assert!(compute_backward_semantic_closure(&[first], 0, &["".into()]).is_err());
}

#[test]
fn requested_read_only_inputs_pull_earlier_writers_without_mutating_original_identity() {
    let writer = summary(&simple("writer", "payer", "fresh", "program"), 0);
    let original = summary(&simple("original", "payer", "old", "program"), 1);
    let mut requested_raw = simple("replacement", "other-payer", "fresh", "program");
    requested_raw["transaction"]["message"]["header"]["numReadonlyUnsignedAccounts"] = json!(2);
    let request = summary(&requested_raw, 1);
    assert!(request.application_writable_accounts.is_empty());
    let target = include_requested_dependencies(&original, &request).expect("merge");
    assert_eq!(target.signature, "original");
    assert_eq!(target.fee_payer, "payer");
    assert!(!original.declared_accounts.contains(&"fresh".into()));
    assert!(target
        .semantic_dependency_accounts
        .contains(&"other-payer".into()));
    let closure = compute_backward_semantic_closure(&[writer, target], 1, &[]).expect("closure");
    assert_eq!(closure.selected_indices, vec![0]);
    for key in ["fresh", "old", "other-payer"] {
        assert!(closure.dependency_accounts.contains(&key.into()));
    }
    let mut wrong = request;
    wrong.index = 2;
    assert!(include_requested_dependencies(&original, &wrong).is_err());
}

#[test]
fn programdata_fixed_point_discovers_cpi_and_transitive_writers() {
    let transactions = [
        semantic(0, &["deep"], &["deep"], "program-0"),
        semantic(1, &["pd-2"], &["pd-2", "deep"], "program-1"),
        semantic(2, &["state"], &["state", "bridge"], "program-2"),
        semantic(3, &[], &["state", "cpi-only"], "target-program"),
    ];
    let mapping = BTreeMap::from([
        ("target-program", "pd-target"),
        ("program-2", "pd-2"),
        ("program-1", "pd-1"),
        ("program-0", "pd-0"),
        ("cpi-only", "pd-cpi"),
    ]);
    let mut batches = Vec::new();
    let closure = compute_program_data_fixed_point(&transactions, 3, |keys| {
        batches.push(keys.to_vec());
        Ok(keys
            .iter()
            .map(|key| {
                (
                    key.clone(),
                    mapping.get(key.as_str()).map(|value| value.to_string()),
                )
            })
            .collect())
    })
    .expect("fixed point");
    assert_eq!(closure.selected_indices, vec![0, 1, 2]);
    assert_eq!(
        closure.program_data_accounts,
        vec!["pd-0", "pd-1", "pd-2", "pd-cpi", "pd-target"]
    );
    assert!(closure.iterations > 1);
    assert!(batches.iter().flatten().any(|key| key == "cpi-only"));
    let flattened: Vec<_> = batches.iter().flatten().collect();
    assert_eq!(
        flattened.iter().collect::<BTreeSet<_>>().len(),
        flattened.len()
    );
}

#[test]
fn programdata_resolver_omissions_errors_empty_addresses_and_nonconvergence_reject() {
    let transactions = [semantic(0, &[], &["state"], "program")];
    assert!(compute_program_data_fixed_point(&transactions, 0, |_| Ok(BTreeMap::new())).is_err());
    let error = compute_program_data_fixed_point(&transactions, 0, |_| {
        Err(svm_replay_protocol::Error::new(
            "SOURCE_UNAVAILABLE",
            "no archive",
        ))
    })
    .expect_err("resolver error");
    assert_eq!(error.code, "SOURCE_UNAVAILABLE");
    assert!(
        compute_program_data_fixed_point(&transactions, 0, |keys| Ok(keys
            .iter()
            .map(|k| (k.clone(), Some(String::new())))
            .collect()))
        .is_err()
    );
    let mut n = 0;
    let error = compute_program_data_fixed_point(&transactions, 0, |keys| {
        n += 1;
        Ok(keys
            .iter()
            .map(|k| (k.clone(), Some(format!("new-{n}-{k}"))))
            .collect())
    })
    .expect_err("bounded nonconvergence");
    assert!(error.message.contains("did not converge"));
    assert_eq!(n, transactions.len() + 2);
}
