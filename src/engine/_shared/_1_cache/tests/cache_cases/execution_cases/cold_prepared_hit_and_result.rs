use super::*;

#[test]
fn cold_prepared_hit_and_result_hit_have_exact_execution_counts() {
    let root = tempfile::tempdir().expect("test root");
    let (request, worker) = fixture(root.path());
    let before = serde_json::to_value(&request).expect("request snapshot");
    let mut store = Store::open(&root.path().join("store")).expect("store");
    let cold = run(root.path(), &mut store, &request, &worker, true).expect("cold verified cache");
    assert_eq!(cold["metrics"]["workerCalls"], 2);
    assert_eq!(cold["metrics"]["prefixTransactionsExecuted"], 1);
    assert_eq!(cold["metrics"]["prefixSimulationCalls"], 1);
    assert_eq!(cold["metrics"]["prefixCommitCalls"], 1);
    let event = &cold["events"][1]["response"];
    assert!(
        event.get("output").is_none(),
        "receipt must not repeat full output"
    );
    let output_bytes = serde_json::to_vec(&cold["output"]).expect("output identity bytes");
    assert_eq!(event["outputSha256"], json!(Digest::of(&output_bytes)));
    assert_eq!(event["outputBytes"], json!(output_bytes.len()));
    assert_eq!(event["metrics"]["prefixTransactionsReused"], 1);
    assert_eq!(event["reads"], json!([]));
    let hit =
        run(root.path(), &mut store, &request, &worker, true).expect("verified cached observation");
    assert_eq!(hit["resultCacheHit"], true);
    assert_eq!(hit["newExecution"], false);
    assert_eq!(hit["metrics"]["workerCalls"], 0);
    assert_eq!(cold["output"], hit["output"]);
    let prepared =
        run(root.path(), &mut store, &request, &worker, false).expect("restored checkpoint");
    assert_eq!(prepared["preparedCacheHit"], true);
    assert_eq!(prepared["metrics"]["workerCalls"], 1);
    assert_eq!(prepared["metrics"]["prefixTransactionsExecuted"], 0);
    assert_eq!(prepared["metrics"]["prefixTransactionsReused"], 1);
    let mut variant = request.clone();
    variant.fixture["target"]["replacementTransactionBase64"] =
        variant.fixture["target"]["transactionBase64"].clone();
    let requested = run(root.path(), &mut store, &variant, &worker, true)
        .expect("variant restores same checkpoint");
    assert_eq!(requested["preparedKey"], cold["preparedKey"]);
    assert_ne!(requested["resultKey"], cold["resultKey"]);
    assert_eq!(requested["metrics"]["workerCalls"], 2);
    assert_eq!(requested["metrics"]["prefixTransactionsExecuted"], 0);
    assert_eq!(
        fs::read_to_string(root.path().join("calls"))
            .expect("real worker call log")
            .lines()
            .count(),
        5
    );
    assert_eq!(
        serde_json::to_value(request).expect("unchanged request"),
        before
    );
    let eviction = store
        .execute(json!({"version":1,"op":"evict","maxBytes":0}))
        .expect("all attempt leases released");
    assert_eq!(eviction["remainingBytes"], 0);
}

#[test]
fn poisoned_results_are_reverified_without_running_workers() {
    let root = tempfile::tempdir().expect("test root");
    let (request, worker) = fixture(root.path());
    let mut store = Store::open(&root.path().join("store")).expect("store");
    let cold = run(root.path(), &mut store, &request, &worker, true).expect("cold run");
    let response = store
        .execute(json!({"version":1,"op":"get","namespace":"result","key":cold["resultKey"]}))
        .expect("stored evidence");
    let mut record: serde_json::Value = serde_json::from_slice(
        &STANDARD
            .decode(response["dataBase64"].as_str().expect("stored bytes"))
            .expect("base64"),
    )
    .expect("computed record");
    record["controlOutput"]["original"]["fee"] = json!("999");
    record["output"]["original"]["fee"] = json!("999");
    let mut poisoned =
        Store::open(&root.path().join("poisoned")).expect("separate adversarial store");
    poisoned.execute(json!({"version":1,"op":"put","namespace":"result","key":cold["resultKey"],"dataBase64":STANDARD.encode(serde_json::to_vec(&record).expect("poisoned record"))})).expect("poison cache under expected key");
    assert_eq!(
        run(root.path(), &mut poisoned, &request, &worker, true)
            .expect_err("archive must refute poisoned observation")
            .code,
        "MISMATCH"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("calls"))
            .expect("worker calls")
            .lines()
            .count(),
        2
    );
}

#[test]
fn corrupt_store_and_mutated_checkpoint_never_fall_back_to_replay() {
    let root = tempfile::tempdir().expect("test root");
    let (request, worker) = fixture(root.path());
    let mut store = Store::open(&root.path().join("store")).expect("store");
    let cold = run(root.path(), &mut store, &request, &worker, false).expect("cold checkpoint");
    fs::write(
        root.path().join("mutate-checkpoint"),
        b"adversarial worker marker",
    )
    .expect("test mutation switch");
    assert_eq!(
        run(root.path(), &mut store, &request, &worker, false)
            .expect_err("checkpoint changed during restore")
            .code,
        "CHECKPOINT_INTEGRITY"
    );
    fs::write(
        root.path()
            .join("store/blobs")
            .join(cold["checkpointSha256"].as_str().expect("checkpoint hash")),
        b"corrupt",
    )
    .expect("corrupt owned test blob");
    let error = run(root.path(), &mut store, &request, &worker, false)
        .expect_err("corrupt blob must fail closed");
    assert!(error.code.starts_with("CACHE_STORE_"), "{error}");
    assert_eq!(
        fs::read_to_string(root.path().join("calls"))
            .expect("worker calls")
            .lines()
            .count(),
        3
    );
}

#[test]
fn lost_cache_lease_is_reported_instead_of_a_success_receipt() {
    let root = tempfile::tempdir().expect("test root");
    let (request, worker) = fixture(root.path());
    let mut store = Store::open(&root.path().join("store")).expect("store");
    run(root.path(), &mut store, &request, &worker, false).expect("cold checkpoint");
    fs::write(
        root.path().join("release-lease"),
        b"adversarial lease owner marker",
    )
    .expect("release fault switch");
    let error = run(root.path(), &mut store, &request, &worker, false)
        .expect_err("lease release lost ownership");
    assert_eq!(error.code, "CACHE_CLEANUP_ERROR");
    assert!(error
        .details
        .expect("cleanup evidence")
        .to_string()
        .contains("NOT_OWNED"));
}
