use super::*;
use serde_json::{json, Value};

const SYSTEM: &str = "11111111111111111111111111111111";
fn fixture() -> (Value, Vec<Value>) {
    let parent = json!({"pubkey":"recipient","presence":"present","owner":SYSTEM,
        "dataBase64":"","executable":false,"lamports":"1000000000",
        "rentEpoch":"18446744073709551615"});
    let data = bs58::encode([2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]).into_string();
    let tx = json!({"transaction":{"signatures":["signature"],"message":{
        "accountKeys":["payer","recipient",SYSTEM],"header":{"numRequiredSignatures":1,
        "numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":1},
        "instructions":[{"programIdIndex":2,"accounts":[0,1],"data":data}]}},
        "meta":{"err":null,"innerInstructions":[],"loadedAddresses":{"writable":[],"readonly":[]},
        "preBalances":[2000000000_u64,1000000000,1],"postBalances":[1999999999_u64,1000000001,1]}});
    (parent, vec![tx])
}
#[test]
fn derives_credit_without_expected_replay_output() {
    let (parent, raw) = fixture();
    let proof = prove(&parent, &raw).unwrap().unwrap();
    assert_eq!(proof["credits"][0]["lamports"], "1");
    assert_eq!(proof["pubkey"], "recipient");
}
#[test]
fn no_fabricated_missing_account_or_foreign_owner() {
    let (mut parent, raw) = fixture();
    parent["presence"] = json!("absent");
    assert!(prove(&parent, &raw).unwrap().is_none());
    parent["presence"] = json!("present");
    parent["owner"] = json!("Vote111111111111111111111111111111111111111");
    assert!(prove(&parent, &raw).unwrap().is_none());
}
#[test]
fn rejects_hidden_lifecycle_and_incomplete_inner_metadata() {
    let (parent, mut raw) = fixture();
    let allocate = bs58::encode([8, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]).into_string();
    raw[0]["meta"]["innerInstructions"] = json!([{"index":0,"instructions":[{
        "programIdIndex":2,"accounts":[1],"data":allocate}]}]);
    assert!(prove(&parent, &raw).unwrap().is_none());
    raw[0]["meta"]["innerInstructions"] = Value::Null;
    assert!(prove(&parent, &raw).unwrap().is_none());
}
#[test]
fn rejects_balance_gap_debit_and_failed_credit() {
    let (parent, raw) = fixture();
    for (field, value) in [
        ("preBalances", json!([2, 7, 1])),
        ("postBalances", json!([2, 999999999, 1])),
    ] {
        let mut changed = raw.clone();
        changed[0]["meta"][field] = value;
        assert!(prove(&parent, &changed).unwrap().is_none());
    }
    let mut changed = raw;
    changed[0]["meta"]["err"] = json!({"InstructionError":[0,"InvalidArgument"]});
    assert!(prove(&parent, &changed).unwrap().is_none());
}

fn planned(late_credit: bool) -> super::CreditPlan {
    use super::super::{compute_backward_semantic_closure, summarize_semantic_transaction};
    let (parent, mut raw) = fixture();
    let mut read = raw[0].clone();
    read["transaction"]["signatures"] = json!(["read"]);
    read["transaction"]["message"]["accountKeys"] = json!(["another-payer", "recipient", "custom"]);
    read["transaction"]["message"]["instructions"] =
        json!([{"programIdIndex":2,"accounts":[1],"data":""}]);
    read["meta"]["preBalances"] = json!([3, 1000000001, 1]);
    read["meta"]["postBalances"] = json!([3, 1000000001, 1]);
    raw.push(read.clone());
    if late_credit {
        let mut credit = raw[0].clone();
        credit["transaction"]["signatures"] = json!(["later-credit"]);
        credit["meta"]["preBalances"] = json!([2000000000, 1000000001, 1]);
        credit["meta"]["postBalances"] = json!([1999999999, 1000000002, 1]);
        raw.push(credit);
    }
    raw.push(read);
    let mut summaries: Vec<_> = raw
        .iter()
        .enumerate()
        .map(|(i, r)| summarize_semantic_transaction(r, i as u64).unwrap())
        .collect();
    // Make the first read a required predecessor through a separate state key.
    summaries[1]
        .application_writable_accounts
        .push("other-state".into());
    let target = summaries.len() - 1;
    summaries[target]
        .semantic_dependency_accounts
        .push("other-state".into());
    let initial = compute_backward_semantic_closure(&summaries, target as u64, &[]).unwrap();
    plan(&[parent], &raw, &summaries, target as u64, &initial).unwrap()
}
#[test]
fn preloads_only_before_first_selected_use() {
    let result = planned(false);
    assert_eq!(result.closure.selected_indices, vec![1]);
    assert_eq!(result.images[0]["lamports"], "1000000001");
    assert_eq!(result.proofs[0]["firstUse"], 1);
}
#[test]
fn retains_dependencies_when_credits_interleave_replay() {
    let result = planned(true);
    assert!(result.images.is_empty());
    assert!(result.closure.selected_indices.contains(&0));
    assert!(result.closure.selected_indices.contains(&2));
}

#[test]
fn prepared_credit_proof_rejects_tampered_initial_balance() {
    let (parent, mut raw) = fixture();
    let mut proof = prove(&parent, &raw).unwrap().unwrap();
    proof["firstUse"] = json!(1);
    proof["appliedCredits"] = proof["credits"].clone();
    proof["initializedLamports"] = json!("1000000001");
    let mut image = parent;
    image["lamports"] = json!("1000000001");
    raw.push(raw[0].clone());
    let mut fixture = json!({"runtime":{"binding":{"executor":{"id":"litesvm-v0.7.1-agave-2.3.9"}},
        "systemCreditPreloads":[proof]},"target":{"index":1,"prefixIndices":[]},"accounts":[image]});
    for executor in [
        "litesvm-v0.7.1-agave-2.3.9",
        "litesvm-v0.8.2-agave-3.0.10",
        "litesvm-v0.12.0-agave-3.1.11",
        "litesvm-v0.13.1-agave-4.0.0",
        "litesvm-v0.14.0-pr402-agave-4.1.2",
        "litesvm-v0.16.0-agave-4.2.1",
    ] {
        fixture["runtime"]["binding"]["executor"]["id"] = json!(executor);
        fixture["accounts"][0]["lamports"] = json!("1000000001");
        validate_preloads(&fixture, &raw).unwrap();
        fixture["accounts"][0]["lamports"] = json!("1000000002");
        assert!(validate_preloads(&fixture, &raw).is_err());
    }
    fixture["accounts"][0]["lamports"] = json!("1000000001");
    fixture["runtime"]["binding"]["executor"]["id"] = json!("unreviewed-executor");
    assert!(validate_preloads(&fixture, &raw).is_err());
}
