use super::*;

#[test]
fn fee_only_shared_payer_does_not_pull_unrelated_predecessor() {
    let transactions = [
        summary(&simple("prefix", "shared", "unrelated", "program-a"), 0),
        summary(&simple("target", "shared", "state", "program-b"), 1),
    ];
    let closure = compute_backward_semantic_closure(&transactions, 1, &[]).expect("closure");
    assert_eq!(
        transactions[0].application_writable_accounts,
        vec!["unrelated"]
    );
    assert!(closure.selected_indices.is_empty());
    assert_eq!(closure.fee_only_payers_ignored, vec!["shared"]);
    assert!(!closure
        .application_dependency_accounts
        .contains(&"shared".into()));
    assert!(closure.dependency_accounts.contains(&"shared".into()));
}

#[test]
fn referenced_payer_and_transitive_writers_are_kept_in_ledger_order() {
    let raws = [
        raw(
            "writer",
            &["payer", "program-a"],
            0,
            json!([{"programIdIndex":1,"accounts":[0]}]),
        ),
        raw(
            "target",
            &["payer", "program-b"],
            0,
            json!([{"programIdIndex":1,"accounts":[0]}]),
        ),
    ];
    let transactions: Vec<_> = raws
        .iter()
        .enumerate()
        .map(|(i, r)| summary(r, i as u64))
        .collect();
    assert_eq!(
        compute_backward_semantic_closure(&transactions, 1, &[])
            .expect("closure")
            .selected_indices,
        vec![0]
    );
    let chain = [
        semantic(2, &["upstream"], &["upstream"], "a"),
        semantic(7, &["state"], &["upstream", "state"], "b"),
        semantic(9, &[], &["state"], "c"),
    ];
    let closure = compute_backward_semantic_closure(&chain, 9, &[]).expect("sparse ledger indexes");
    assert_eq!(closure.selected_indices, vec![2, 7]);
    assert_eq!(closure.writable_intersections[0].accounts, vec!["upstream"]);
}

#[test]
fn outer_program_writable_demotion_does_not_demote_cpi_only_programs() {
    let outer = summary(
        &raw(
            "outer",
            &["payer", "state", "program"],
            2,
            json!([{"programIdIndex":2,"accounts":[1,2]}]),
        ),
        0,
    );
    assert_eq!(outer.writable_accounts, vec!["payer", "state"]);
    assert_eq!(outer.application_writable_accounts, vec!["state"]);
    let mut cpi = raw(
        "cpi",
        &["payer", "state", "cpi-program", "outer-program"],
        2,
        json!([{"programIdIndex":3,"accounts":[1,2]}]),
    );
    cpi["meta"]["innerInstructions"] =
        json!([{"index":0,"instructions":[{"programIdIndex":2,"accounts":[0,1]}]}]);
    let cpi = summary(&cpi, 0);
    assert_eq!(cpi.program_ids, vec!["outer-program", "cpi-program"]);
    assert_eq!(cpi.writable_accounts, vec!["payer", "state", "cpi-program"]);
    assert!(!cpi.fee_only_payer);
    let loader = summary(
        &raw(
            "loader",
            &[
                "payer",
                "state",
                "program",
                "BPFLoaderUpgradeab1e11111111111111111111111",
            ],
            2,
            json!([{"programIdIndex":2,"accounts":[1,2]}]),
        ),
        0,
    );
    assert_eq!(
        loader.application_writable_accounts,
        vec!["state", "program"]
    );
}

#[test]
fn loaded_addresses_alias_positions_tables_and_parsed_pubkeys_are_retained() {
    let mut value = raw(
        "v0",
        &["payer", "program"],
        0,
        json!([{"programIdIndex":1,"accounts":[3,2,2,"payer"]}]),
    );
    value["transaction"]["message"]["accountKeys"] = json!([{"pubkey":"payer"},"program"]);
    value["transaction"]["message"]["addressTableLookups"] =
        json!([{"accountKey":"table"},{"accountKey":{"pubkey":"table"}}]);
    value["meta"]["loadedAddresses"] = json!({"writable":["loaded"],"readonly":["loaded"]});
    let transaction = summary(&value, 0);
    assert_eq!(
        transaction.declared_accounts,
        vec!["payer", "program", "loaded"]
    );
    assert_eq!(transaction.instruction_accounts, vec!["loaded", "payer"]);
    assert_eq!(transaction.writable_accounts, vec!["payer", "loaded"]);
    assert_eq!(transaction.address_table_accounts, vec!["table"]);
    assert_eq!(
        transaction.semantic_dependency_accounts,
        vec!["loaded", "payer", "table", "program"]
    );
}

#[test]
fn failed_application_writes_rollback_but_first_nonce_candidates_remain() {
    let mut failed = simple("failed", "payer-0", "state", "program-a");
    failed["meta"]["err"] = json!({"InstructionError":[0,{"Custom":7}]});
    let transactions = [
        summary(&failed, 0),
        summary(&simple("target", "payer-1", "state", "program-b"), 1),
    ];
    assert!(compute_backward_semantic_closure(&transactions, 1, &[])
        .expect("rollback")
        .selected_indices
        .is_empty());
    for (first, trailing, expected) in [
        (true, false, vec![0]),
        (false, false, vec![]),
        (true, true, vec![0]),
    ] {
        let transactions = [
            summary(&nonce(first, trailing), 0),
            summary(&simple("target", "payer", "nonce", SYSTEM), 1),
        ];
        let closure =
            compute_backward_semantic_closure(&transactions, 1, &[]).expect("nonce closure");
        assert_eq!(closure.selected_indices, expected);
        assert_eq!(transactions[0].possible_persistent_writes.is_some(), first);
    }
    let transactions = [
        summary(&nonce(true, false), 0),
        summary(&simple("target", "payer", "recipient", SYSTEM), 1),
    ];
    assert!(compute_backward_semantic_closure(&transactions, 1, &[])
        .expect("recipient rolled back")
        .selected_indices
        .is_empty());
}

#[test]
fn votes_and_excluded_runtime_inputs_are_reported_not_discarded() {
    let vote = "Vote111111111111111111111111111111111111111";
    let writer = summary(&simple("vote", "vote-payer", "state", vote), 0);
    let target = summary(&simple("target", "payer", "state", "program"), 1);
    let closure =
        compute_backward_semantic_closure(&[writer, target], 1, &[RECENT.into()]).expect("closure");
    assert_eq!(closure.included_vote_indices, vec![0]);
    assert_eq!(closure.excluded_dependency_accounts, vec![RECENT, vote]);
    assert!(closure.dependency_accounts.contains(&vote.into()));
}
