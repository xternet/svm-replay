use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};
use svm_replay_engine::{
    _2_prepare_state::source::reconstruct,
    shared::{
        history::History,
        runtime::{read_bounded_file, CancellationToken, ExecutionBudget},
        sources::{CapturedSource, CompositeSource},
    },
};
use svm_replay_protocol::{parse_json, Digest, HistoricalRequest};
fn read(path: &Path) -> Value {
    parse_json(&read_bounded_file(path, 256 * 1024 * 1024).unwrap()).unwrap()
}
fn by_key(accounts: &Value) -> BTreeMap<String, Value> {
    accounts
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["pubkey"].as_str().unwrap().into(), a.clone()))
        .collect()
}
#[test]
#[ignore = "explicit read-only preserved source corpus; SVM_REPLAY_TEST_SOURCE_CASE and SVM_REPLAY_TEST_ARTIFACT_ROOT required"]
fn reconstruct_from_real_captured_inputs_without_the_typescript_coordinator() {
    let case = std::env::var("SVM_REPLAY_TEST_SOURCE_CASE").expect("explicit source case required");
    let artifacts =
        std::env::var("SVM_REPLAY_TEST_ARTIFACTS").expect("reviewed artifact directory");
    let root = Path::new(&artifacts)
        .join("m11-historical-final-jWj9Uy")
        .join(&case);
    let manifest = root.join("captured/manifest.json");
    let bytes = std::fs::read(&manifest).unwrap();
    let source_pin = Digest::of(&bytes);
    let manifest_value = parse_json(&bytes).unwrap();
    let expected = read(&root.join("prepared.json"));
    let corpus = read(
        &Path::new(&artifacts)
            .join("m12-sealed-historical-XmOnk2/historical/regression-inputs.json"),
    );
    let candidate = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["candidate"]["id"] == case)
        .unwrap()["candidate"]
        .clone();
    let family = match candidate["executorSourceId"].as_str().unwrap() {
        "litesvm-v0.7.1-agave-2.3.9" => "v2-3",
        "litesvm-v0.8.2-agave-3.0.10" => "v3-0",
        "litesvm-v0.12.0-agave-3.1.11" => "v3-1",
        "litesvm-v0.13.1-agave-4.0.0" => "v4-0",
        "litesvm-v0.14.0-pr402-agave-4.1.2" => "v4-1",
        "litesvm-v0.16.0-agave-4.2.1" => "v4-2",
        other => panic!("unreviewed {other}"),
    };
    let input = json!({"schema":"svm-replay-historical/v1","requestId":case,"family":family,
        "genesisHash":manifest_value["identity"]["genesisHash"],"candidate":candidate,"runtimeBinding":expected["runtime"]["binding"],
        "replacementTransactionBase64":null,"requestedAccountOverrides":null,"bankInputs":[],"metadataPolicy":"ARCHIVED_COMPUTE_METER_WARNING",
        "limits":{"timeoutMs":300000,"maxOutputBytes":67108864,"maxDiagnosticBytes":1048576}});
    let input = HistoricalRequest::parse(&serde_json::to_vec(&input).unwrap()).unwrap();
    let mut sources = CompositeSource::new(vec![Box::new(
        CapturedSource::open(&manifest, &source_pin).unwrap(),
    )])
    .unwrap();
    let budget = ExecutionBudget::new(Duration::from_secs(300), CancellationToken::new()).unwrap();
    let mut history = History::new(
        &mut sources,
        input.genesis_hash.clone(),
        input.candidate["slot"].as_u64().unwrap(),
        &budget,
        10000,
    )
    .unwrap();
    let start = Instant::now();
    let prepared = reconstruct(&input, &mut history).unwrap();
    let out = std::env::var("SVM_REPLAY_TEST_ARTIFACT_ROOT")
        .expect("explicit authorized artifact root required");
    let dir = tempfile::Builder::new()
        .prefix(&format!("source-{case}-"))
        .tempdir_in(out)
        .unwrap()
        .keep();
    svm_replay_engine::_5_finalize::write_json(
        &dir.join("historical-request.json"),
        &serde_json::to_value(&input).unwrap(),
    )
    .unwrap();
    svm_replay_engine::_5_finalize::write_json(
        &dir.join("prepared-request.json"),
        &serde_json::to_value(&prepared.request).unwrap(),
    )
    .unwrap();
    svm_replay_engine::_5_finalize::write_json(
        &dir.join("preparation-receipt.json"),
        &prepared.receipt,
    )
    .unwrap();
    // Expected bytes are used only by this independent assertion, never by reconstruction.
    assert!(
        prepared.request.fixture["target"] == expected["target"],
        "target differs: {}",
        dir.display()
    );
    assert_eq!(prepared.request.fixture["clock"], expected["clock"]);
    assert!(
        by_key(&prepared.request.fixture["accounts"]) == by_key(&expected["accounts"]),
        "parent images differ: {}",
        dir.display()
    );
    assert!(
        by_key(&prepared.request.fixture["endAccounts"]) == by_key(&expected["endAccounts"]),
        "end images differ: {}",
        dir.display()
    );
    // New guarded declarations change the derived context hash, not historical
    // Bank facts. Compare every other field and every non-generic requirement.
    let mut actual_bank = prepared.request.fixture["runtime"]["bankContext"].clone();
    let mut expected_bank = expected["runtime"]["bankContext"].clone();
    for bank in [&mut actual_bank, &mut expected_bank] {
        bank.as_object_mut().unwrap().remove("derivationHash");
        bank["requiredRuntimeSysvars"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["requirement"] != "tracked-generic-sysvar-context-v1");
    }
    assert_eq!(actual_bank, expected_bank);
    assert_eq!(
        prepared.request.fixture["runtime"]["bankContext"]["derivationHash"],
        svm_replay_engine::shared::bank::context::historical_bank_context_hash(
            &prepared.request.fixture["runtime"]["bankContext"]
        )
        .unwrap()
    );
    assert_eq!(Digest::of(std::fs::read(&manifest).unwrap()), source_pin);
    println!(
        "{}",
        json!({"caseId":case,"sourcePreparation":"PASS","seconds":start.elapsed().as_secs_f64(),"artifacts":dir,
        "prefixCount":prepared.request.fixture["target"]["prefixIndices"].as_array().unwrap().len(),"networkRequests":0})
    );
}
