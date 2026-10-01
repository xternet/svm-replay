use serde_json::json;
use svm_replay_engine::shared::dependencies::{
    compute_backward_semantic_closure, summarize_semantic_transaction,
};
use svm_replay_engine::shared::fees::{derive_omitted_fee_effects, eligible_end_accounts};

fn tx(payer: &str, read: &str, failed: bool) -> serde_json::Value {
    json!({"transaction":{"signatures":[format!("sig-{payer}")],"message":{
      "accountKeys":[payer,read,"11111111111111111111111111111111"],"header":{"numRequiredSignatures":1,"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":1},
      "instructions":[{"programIdIndex":2,"accounts":[0,1],"data":""}],"addressTableLookups":[]}},
      "meta":{"err":if failed {json!({"InstructionError":[0,"Custom"]})}else{json!(null)},"fee":3,
      "preBalances":[10,20,0],"postBalances":[7,20,0],"innerInstructions":[],"loadedAddresses":{"writable":[],"readonly":[]}}})
}

fn passive_payer(payer: &str, application: &str) -> serde_json::Value {
    let mut value = tx(payer, application, false);
    value["transaction"]["message"]["instructions"][0]["accounts"] = json!([1]);
    value
}

#[test]
fn omitted_fees_still_affect_a_passive_target_payer() {
    let raw = vec![
        passive_payer("A", "unrelated"),
        passive_payer("A", "target"),
    ];
    let summaries = raw
        .iter()
        .enumerate()
        .map(|(i, value)| summarize_semantic_transaction(value, i as u64).unwrap())
        .collect::<Vec<_>>();
    let closure = compute_backward_semantic_closure(&summaries, 1, &[]).unwrap();
    assert!(closure.selected_indices.is_empty());
    let effects = derive_omitted_fee_effects(&raw, &summaries, 1, &closure).unwrap();
    assert_eq!(effects["omittedFeeEffects"].as_array().unwrap().len(), 1);
    assert_eq!(effects["omittedFeeEffects"][0]["feePayer"], "A");
}

#[test]
fn selected_prefix_passive_payer_keeps_prior_fees_not_later_ones() {
    let raw = vec![
        passive_payer("A", "unrelated"),
        passive_payer("A", "shared"),
        passive_payer("A", "later"),
        tx("target", "shared", false),
    ];
    let summaries = raw
        .iter()
        .enumerate()
        .map(|(i, value)| summarize_semantic_transaction(value, i as u64).unwrap())
        .collect::<Vec<_>>();
    let closure = compute_backward_semantic_closure(&summaries, 3, &[]).unwrap();
    assert_eq!(closure.selected_indices, vec![1]);
    let effects = derive_omitted_fee_effects(&raw, &summaries, 3, &closure).unwrap();
    assert_eq!(effects["omittedFeeEffects"].as_array().unwrap().len(), 1);
    assert_eq!(effects["omittedFeeEffects"][0]["index"], 0);
    let mut invalid = raw.clone();
    invalid[0]["meta"]["postBalances"][0] = json!(6);
    assert!(derive_omitted_fee_effects(&invalid, &summaries, 3, &closure).is_err());
}
#[test]
fn failed_omitted_payer_fee_survives_rollback() {
    let raw = vec![tx("A", "B", true), tx("C", "A", false)];
    let summaries = raw
        .iter()
        .enumerate()
        .map(|(i, t)| summarize_semantic_transaction(t, i as u64).unwrap())
        .collect::<Vec<_>>();
    let closure = compute_backward_semantic_closure(&summaries, 1, &[]).unwrap();
    assert!(closure.selected_indices.is_empty());
    let effects = derive_omitted_fee_effects(&raw, &summaries, 1, &closure).unwrap();
    assert_eq!(effects["omittedFeeEffects"][0]["feeLamports"], "3");
    assert_eq!(effects["omittedFeeEffects"][0]["preLamports"], "10");
    let mut corrupt = raw.clone();
    corrupt[0]["meta"]["postBalances"][1] = json!(19);
    assert!(derive_omitted_fee_effects(&corrupt, &summaries, 1, &closure).is_err());
}
#[test]
fn absent_fee_witness_and_bad_fee_delta_are_errors() {
    let raw = vec![tx("A", "B", true), tx("C", "A", false)];
    let summaries = raw
        .iter()
        .enumerate()
        .map(|(i, t)| summarize_semantic_transaction(t, i as u64).unwrap())
        .collect::<Vec<_>>();
    let closure = compute_backward_semantic_closure(&summaries, 1, &[]).unwrap();
    for value in [json!(0), json!(4), json!(-1), json!(null)] {
        let mut altered = raw.clone();
        altered[0]["meta"]["fee"] = value;
        assert!(derive_omitted_fee_effects(&altered, &summaries, 1, &closure).is_err());
    }
}
#[test]
fn later_failed_fee_write_disqualifies_end_balance_as_earlier_evidence() {
    let raw = vec![tx("A", "B", false), tx("A", "C", true)];
    let summaries = raw
        .iter()
        .enumerate()
        .map(|(i, t)| summarize_semantic_transaction(t, i as u64).unwrap())
        .collect::<Vec<_>>();
    let eligible = eligible_end_accounts(&summaries, &[0]).unwrap();
    assert!(!eligible.contains(&"A".to_owned()));
    assert!(eligible.contains(&"B".to_owned()));
}

#[test]
fn readonly_vote_authority_retains_its_omitted_fee() {
    let mut vote = tx("authority", "vote-state", false);
    vote["transaction"]["message"]["accountKeys"][2] =
        json!("Vote111111111111111111111111111111111111111");
    vote["transaction"]["message"]["instructions"][0] = json!({
        "programIdIndex":2,"accounts":[1,0],
        "data":bs58::encode(14u32.to_le_bytes()).into_string()
    });
    let raw = vec![vote, tx("payer", "authority", false)];
    let summaries = raw
        .iter()
        .enumerate()
        .map(|(i, value)| summarize_semantic_transaction(value, i as u64).unwrap())
        .collect::<Vec<_>>();
    let closure = compute_backward_semantic_closure(&summaries, 1, &[]).unwrap();
    assert!(closure.selected_indices.is_empty());
    let effects = derive_omitted_fee_effects(&raw, &summaries, 1, &closure).unwrap();
    assert_eq!(effects["omittedFeeEffects"][0]["feePayer"], "authority");
    assert_eq!(effects["omittedFeeEffects"][0]["feeLamports"], "3");
    let mut altered = raw.clone();
    altered[0]["meta"]["postBalances"][0] = json!(6);
    assert!(derive_omitted_fee_effects(&altered, &summaries, 1, &closure).is_err());
}
