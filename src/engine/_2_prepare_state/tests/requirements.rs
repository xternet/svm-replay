use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use svm_replay_engine::shared::{
    history::RECENT_BLOCKHASHES,
    instructions::{requirements, resolve},
};
fn raw() -> serde_json::Value {
    json!({"transaction":{"message":{"accountKeys":["payer",RECENT_BLOCKHASHES,"11111111111111111111111111111111"],
        "instructions":[{"programIdIndex":2,"accounts":[0,1],"data":bs58::encode([4,0,0,0]).into_string()}]}},
        "meta":{"loadedAddresses":{"writable":[],"readonly":[]},"innerInstructions":[]}})
}
#[test]
fn only_outer_nonce_advance_can_use_synthesized_recent_blockhashes() {
    let original = raw();
    let requirements = requirements(&[original.clone()], &[0], 0).unwrap();
    assert_eq!(requirements["recentBlockhashes"]["runtimeResolvable"], true);
    assert_eq!(requirements["exactAccountRequired"], json!([]));
    let mut inner = original.clone();
    inner["meta"]["innerInstructions"] =
        json!([{"index":0,"instructions":inner["transaction"]["message"]["instructions"]}]);
    assert_eq!(
        requirements_fn(&inner)["recentBlockhashes"]["runtimeResolvable"],
        false
    );
    for tag in [1u8, 3, 5, 7] {
        let mut different = original.clone();
        different["transaction"]["message"]["instructions"][0]["data"] =
            json!(bs58::encode([tag, 0, 0, 0]).into_string());
        assert_eq!(
            requirements_fn(&different)["exactAccountRequired"],
            json!([RECENT_BLOCKHASHES])
        );
    }
}
fn requirements_fn(raw: &serde_json::Value) -> serde_json::Value {
    requirements(&[raw.clone()], &[0], 0).unwrap()
}
#[test]
fn cpi_metadata_must_be_complete_unique_and_bound_to_message() {
    let original = raw();
    assert_eq!(resolve(&original).unwrap().len(), 1);
    for groups in [
        serde_json::Value::Null,
        json!([{"index":1,"instructions":[]}]),
        json!([{"index":0,"instructions":[]},{"index":0,"instructions":[]}]),
    ] {
        let mut invalid = original.clone();
        invalid["meta"]["innerInstructions"] = groups;
        assert!(resolve(&invalid).is_err());
    }
    let mut invalid = original.clone();
    invalid["transaction"]["message"]["instructions"][0]["accounts"] = json!(["undeclared"]);
    assert!(resolve(&invalid).is_err());
    invalid = original.clone();
    invalid["transaction"]["message"]["instructions"][0]["programIdIndex"] = json!(4);
    assert!(resolve(&invalid).is_err());
    // Ensure unrelated bytes cannot masquerade as base58 instruction data.
    invalid = original;
    invalid["transaction"]["message"]["instructions"][0]["data"] =
        json!(STANDARD.encode([255u8; 4]));
    assert!(resolve(&invalid).is_err());
}

#[test]
fn proven_preexecution_data_limit_failure_has_no_cpi_recording() {
    let mut transaction = raw();
    transaction["meta"]["err"] = json!("MaxLoadedAccountsDataSizeExceeded");
    transaction["meta"]["innerInstructions"] = serde_json::Value::Null;
    transaction["meta"]["logMessages"] = serde_json::Value::Null;
    transaction["meta"]["computeUnitsConsumed"] = json!(0);
    assert_eq!(resolve(&transaction).unwrap().len(), 1);
    transaction["meta"]["computeUnitsConsumed"] = json!(1);
    assert!(resolve(&transaction).is_err());
    transaction["meta"]["computeUnitsConsumed"] = json!(0);
    transaction["meta"]["returnData"] = json!({"data":"unexpected"});
    assert!(resolve(&transaction).is_err());
}

fn introspection(declared: bool) -> serde_json::Value {
    let mut value = raw();
    value["transaction"]["signatures"] = json!(["instructions-usage"]);
    value["transaction"]["message"]["header"] = json!({"numRequiredSignatures":1,"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":2});
    value["meta"]["err"] = serde_json::Value::Null;
    if declared {
        value["transaction"]["message"]["accountKeys"][1] =
            json!("Sysvar1nstructions1111111111111111111111111");
    }
    value
}
#[test]
fn instructions_sysvar_proof_counts_instructions_not_duplicate_account_positions() {
    use svm_replay_engine::shared::instructions::analyze_instructions_sysvar;
    let mut prefix = introspection(true);
    prefix["transaction"]["message"]["instructions"][0]["accounts"] = json!([1, 1]);
    let mut target = introspection(true);
    let inner = target["transaction"]["message"]["instructions"][0].clone();
    target["meta"]["innerInstructions"] = json!([{"index":0,"instructions":[inner]}]);
    let raw = vec![prefix, introspection(false), target];
    let before = raw.clone();
    assert_eq!(
        analyze_instructions_sysvar(&raw, &[0, 1], 2).unwrap(),
        json!({
        "pubkey":"Sysvar1nstructions1111111111111111111111111","requirement":"per-transaction-message-account-v1",
        "declaredTransactionIndices":[0,2],"useCount":3,"targetUses":2,"runtimeSynthesized":true,"exactAccountRequired":false})
    );
    assert_eq!(raw, before);
}
#[test]
fn instructions_sysvar_absence_and_loaded_inner_only_use_are_distinct() {
    use svm_replay_engine::shared::instructions::analyze_instructions_sysvar;
    let absent = introspection(false);
    assert_eq!(
        analyze_instructions_sysvar(std::slice::from_ref(&absent), &[], 0).unwrap(),
        json!({
        "pubkey":"Sysvar1nstructions1111111111111111111111111","requirement":"per-transaction-message-account-v1",
        "declaredTransactionIndices":[],"useCount":0,"targetUses":0,"runtimeSynthesized":true,"exactAccountRequired":false})
    );
    let mut loaded = absent.clone();
    loaded["version"] = json!(0);
    loaded["transaction"]["message"]["addressTableLookups"] =
        json!([{"accountKey":"table","writableIndexes":[],"readonlyIndexes":[0]}]);
    loaded["meta"]["loadedAddresses"]["readonly"] =
        json!(["Sysvar1nstructions1111111111111111111111111"]);
    loaded["meta"]["innerInstructions"] =
        json!([{"index":0,"instructions":[{"programIdIndex":2,"accounts":[3],"data":""}]}]);
    let proof = analyze_instructions_sysvar(&[loaded, absent], &[0], 1).unwrap();
    assert_eq!(proof["declaredTransactionIndices"], json!([0]));
    assert_eq!(proof["useCount"], 1);
    assert_eq!(proof["targetUses"], 0);
}
#[test]
fn instructions_sysvar_rejects_unused_declaration_overlapping_and_missing_transactions() {
    use svm_replay_engine::shared::instructions::analyze_instructions_sysvar;
    let mut value = introspection(true);
    value["transaction"]["message"]["instructions"][0]["accounts"] = json!([0]);
    let error = analyze_instructions_sysvar(std::slice::from_ref(&value), &[], 0).unwrap_err();
    assert!(error
        .message
        .contains("transaction 0: declared Instructions account is unused"));
    let raw = vec![introspection(false), introspection(true)];
    assert!(analyze_instructions_sysvar(&raw, &[1], 1).is_err());
    assert!(analyze_instructions_sysvar(&raw, &[0, 0], 1).is_err());
    assert!(analyze_instructions_sysvar(&raw, &[3], 1).is_err());
    assert!(analyze_instructions_sysvar(&raw, &[], 3).is_err());
    let mut malformed = introspection(true);
    malformed["meta"]["innerInstructions"] = serde_json::Value::Null;
    assert!(analyze_instructions_sysvar(&[malformed], &[], 0).is_err());
}
