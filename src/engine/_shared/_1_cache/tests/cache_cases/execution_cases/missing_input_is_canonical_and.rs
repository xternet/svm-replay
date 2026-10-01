use super::*;

#[test]
fn missing_input_is_canonical_and_never_publishes_partial_attempts() {
    for phase in ["all", "requested"] {
        let root = tempfile::tempdir().expect("test root");
        let (mut request, worker) = fixture(root.path());
        request.fixture["target"]["replacementTransactionBase64"] =
            request.fixture["target"]["transactionBase64"].clone();
        fs::write(root.path().join("needs-input"), phase).expect("guarded input switch");
        let mut store = Store::open(&root.path().join("store")).expect("store");
        let error = run(root.path(), &mut store, &request, &worker, true)
            .expect_err("incomplete guarded execution");
        assert_eq!(error.code, "NEEDS_INPUT");
        let details = error.details.expect("retry evidence");
        assert_eq!(
            details["cause"],
            json!({"schema":"svm-sysvar-discovery/v1","status":"NEEDS_INPUT",
            "pubkey":"SysvarRent111111111111111111111111111111111",
            "reads":[{"pubkey":"SysvarRent111111111111111111111111111111111","offset":"0","length":"8"}]})
        );
        assert_eq!(
            details["events"].as_array().expect("events").len(),
            if phase == "all" { 1 } else { 3 }
        );
        if phase == "requested" {
            let evidence = &details["controlEvidence"];
            assert_eq!(evidence["phase"], "original-control");
            assert_eq!(evidence["verification"]["status"], "PASS");
            assert_eq!(evidence["complete"], true);
            assert_eq!(
                evidence["outputSha256"],
                details["events"][1]["response"]["outputSha256"]
            );
            let canonical =
                svm_replay_engine::_2_prepare_state::original_control(&request.fixture).unwrap();
            assert_eq!(
                evidence["fixtureSha256"],
                json!(Digest::of(svm_replay_engine::shared::diff::canonical_json(
                    &canonical
                )))
            );
            assert!(evidence.get("output").is_none());
        } else {
            assert!(details.get("controlEvidence").is_none());
        }
        for (namespace, key) in [("prepared", "preparedKey"), ("result", "resultKey")] {
            let lookup = store
                .execute(json!({"version":1,"op":"get","namespace":namespace,"key":details[key]}))
                .expect("inspect unpublished entry");
            assert_eq!(lookup["status"], "MISS");
        }
    }
}

#[test]
#[ignore = "requires explicitly selected historical input, pinned catalog/owner, and approved artifact root"]
fn real_historical_checkpoint_cold_prepared_and_result_hits() {
    use svm_replay_engine::_1_resolve_runtime::{load_catalog, resolve};
    use svm_replay_engine::shared::runtime::{read_bounded_file, ProcessOwner};
    let required = |name: &str| {
        std::env::var(name).unwrap_or_else(|error| panic!("{name} is required: {error}"))
    };
    let request_path = std::path::PathBuf::from(required("SVM_REPLAY_TEST_CACHE_REQUEST"));
    let catalog_path = std::path::PathBuf::from(required("SVM_REPLAY_TEST_CACHE_CATALOG"));
    let catalog_hash = Digest::new(required("SVM_REPLAY_TEST_CACHE_CATALOG_SHA256"))
        .expect("external catalog pin");
    let request = PreparedRequest::parse(
        &read_bounded_file(&request_path, 256 * 1024 * 1024).expect("historical input"),
    )
    .expect("admitted historical request");
    let catalog =
        load_catalog(&catalog_path, &catalog_hash).expect("trusted installed worker catalog");
    let worker = resolve(&catalog, &request).expect("exact historical worker");
    let owner = ProcessOwner {
        executable: required("SVM_REPLAY_TEST_CACHE_OWNER").into(),
        sha256: required("SVM_REPLAY_TEST_CACHE_OWNER_SHA256"),
    };
    let implementation = Digest::new(
        file_sha256(&std::env::current_exe().expect("native test consumer executable"))
            .expect("actual consumer implementation hash"),
    )
    .expect("implementation pin");
    let work = tempfile::Builder::new()
        .prefix("cache-history-")
        .tempdir_in(required("SVM_REPLAY_TEST_CACHE_WORK_ROOT"))
        .expect("approved artifact root")
        .keep();
    println!("historical cache artifact directory: {}", work.display());
    let mut store = Store::open(&work.join("store")).expect("fresh historical cache");
    let transport = WorkerTransport::new(&work).with_owner(owner);
    let limits = WorkerLimits {
        max_input_bytes: 256 * 1024 * 1024,
        max_output_bytes: request.limits.max_output_bytes as usize,
        max_diagnostic_bytes: request.limits.max_diagnostic_bytes as usize,
        cleanup_grace: Duration::from_secs(2),
    };
    let mut receipts = Vec::new();
    for (name, result_cache) in [
        ("cold", true),
        ("result-hit", true),
        ("prepared-hit", false),
    ] {
        let receipt = execute(
            &request,
            &worker,
            CacheContext {
                store: &mut store,
                transport: &transport,
                scratch_root: &work,
                catalog_sha256: &catalog_hash,
                implementation_sha256: &implementation,
            },
            &limits,
            &ExecutionBudget::new(
                Duration::from_millis(request.limits.timeout_ms),
                CancellationToken::new(),
            )
            .expect("shared attempt budget"),
            CacheOptions { result_cache },
        );
        match receipt {
            Ok(receipt) => {
                fs::write(
                    work.join(format!("{name}.json")),
                    serde_json::to_vec(&receipt).expect("cache receipt JSON"),
                )
                .expect("retain historical receipt");
                println!("{name}: {}", receipt["metrics"]);
                receipts.push(receipt);
            }
            Err(error) => {
                fs::write(
                    work.join(format!("{name}-failure.json")),
                    serde_json::to_vec(&error).expect("failure JSON"),
                )
                .expect("retain failure evidence");
                panic!("historical {name} failed: {error}");
            }
        }
    }
    assert_eq!(receipts[0]["output"], receipts[1]["output"]);
    assert_eq!(receipts[0]["output"], receipts[2]["output"]);
    assert_eq!(receipts[1]["metrics"]["workerCalls"], 0);
    assert_eq!(receipts[1]["newExecution"], false);
    assert_eq!(receipts[2]["preparedCacheHit"], true);
    assert_eq!(receipts[2]["metrics"]["prefixTransactionsExecuted"], 0);
    assert_eq!(
        receipts[0]["metrics"]["prefixTransactionsExecuted"]
            .as_u64()
            .expect("observed prefix count"),
        request.fixture["target"]["prefixIndices"]
            .as_array()
            .expect("input predecessor set")
            .len() as u64
    );
    fs::write(work.join("request-identity.json"),serde_json::to_vec(&json!({"requestPath":request_path,"requestSha256":file_sha256(&request_path).expect("input artifact hash"),
        "catalogPath":catalog_path,"catalogSha256":catalog_hash,"implementationSha256":implementation,"worker":worker.descriptor})).expect("identity evidence")).expect("retain input identities");
}
