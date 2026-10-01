use super::*;

#[test]
#[ignore = "requires SVM_REPLAY_TEST_BANK_M6_ROOT and SVM_REPLAY_TEST_BANK_BLOCK_ROOT preserved inputs"]
fn preserved_bank_context_and_restart_hashes_recompute() {
    let root = std::path::PathBuf::from(
        std::env::var("SVM_REPLAY_TEST_BANK_M6_ROOT").expect("SVM_REPLAY_TEST_BANK_M6_ROOT"),
    );
    let blocks = std::path::PathBuf::from(
        std::env::var("SVM_REPLAY_TEST_BANK_BLOCK_ROOT").expect("SVM_REPLAY_TEST_BANK_BLOCK_ROOT"),
    );
    let read = |path: &std::path::Path| -> Value {
        svm_replay_protocol::parse_json(&std::fs::read(path).unwrap()).unwrap()
    };
    let plan = read(&root.join("m6-plan.json"));
    let candidate = &plan["cases"][0];
    let case = root
        .join("cases")
        .join(candidate["id"].as_str().unwrap())
        .join("attempt-1");
    let fixture = read(&case.join("fixture.json"));
    let transcript = read(&case.join("last-restart-transcript.json"));
    let block = read(&blocks.join(candidate["blockFile"].as_str().unwrap()));
    let selector = json!({"slot":candidate["slot"],"parentSlot":fixture["target"]["parentSlot"],"blockhash":block["result"]["blockhash"],
        "index":candidate["transactionIndex"],"signature":candidate["targetSignature"]});
    let prefix = fixture["target"]["prefixIndices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect::<Vec<_>>();
    let context = derive_historical_bank_context(
        &block,
        &selector,
        candidate["blockSourceHash"].as_str().unwrap(),
        &prefix,
    )
    .unwrap();
    assert_eq!(context, fixture["runtime"]["bankContext"]);
    let restart = prepare_last_restart_slot(
        &transcript,
        fixture["target"]["parentSlot"].as_u64().unwrap(),
    )
    .unwrap();
    assert_eq!(restart["binding"], fixture["runtime"]["lastRestartSlot"]);
}
