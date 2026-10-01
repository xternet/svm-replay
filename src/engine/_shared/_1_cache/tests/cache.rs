use serde_json::json;
use svm_replay_engine::shared::cache::boundary::{checkpoint_response, CheckpointMode};
use svm_replay_engine::shared::cache::identity::derive;
use svm_replay_protocol::{worker::WorkerDescriptor, Digest, PreparedRequest};

fn identity_input() -> (PreparedRequest, WorkerDescriptor) {
    let identity = Digest::of(b"synthetic cache identity");
    let request = serde_json::from_value(json!({"schema":"svm-replay-prepared/v1","requestId":"test","family":"v2-3",
        "candidate":{"slot":20,"transactionIndex":1},"fixture":{"target":{"prefixIndices":[0],"replacementTransactionBase64":null},
        "runtime":{"profileHash":identity,"genericSysvars":[]},"accountOverride":null,"accounts":[],"endAccounts":[]},
        "rawBlockBase64":"e30=","blockSha256":Digest::of(b"{}"),"sourceEvidenceHashes":[identity],
        "metadataPolicy":"STRICT","limits":{"timeoutMs":10000,"maxOutputBytes":4096,"maxDiagnosticBytes":4096}})).expect("identity test request");
    let worker = serde_json::from_value(json!({"family":"v2-3","file":"worker","sha256":identity,"buildHash":identity,
        "sourceSha256":identity,"executorSourceId":"synthetic","capabilities":["historical-boundary-checkpoint/v1","guarded-generic-sysvars/v1"]})).expect("worker descriptor");
    (request, worker)
}

#[test]
fn cache_identity_separates_variants_and_binds_all_historical_inputs() {
    let (request, worker) = identity_input();
    let catalog = Digest::of(b"catalog");
    let implementation = Digest::of(b"native implementation");
    let original = derive(&request, &worker, &catalog, &implementation).expect("identity");
    let mut variant = request.clone();
    variant.fixture["accountOverride"] = json!({"pubkey":"account","lamports":"2"});
    let changed = derive(&variant, &worker, &catalog, &implementation).expect("variant identity");
    assert_eq!(changed.prepared_key, original.prepared_key);
    assert_ne!(changed.result_key, original.result_key);
    for pointer in [
        "/runtime/profileHash",
        "/runtime/genericSysvars",
        "/accounts",
        "/endAccounts",
    ] {
        let mut changed = request.clone();
        *changed
            .fixture
            .pointer_mut(pointer)
            .expect("known identity field") = json!("changed");
        assert_ne!(
            derive(&changed, &worker, &catalog, &implementation)
                .expect("changed identity")
                .prepared_key,
            original.prepared_key,
            "{pointer}"
        );
    }
    let mut evidence = request.clone();
    evidence
        .source_evidence_hashes
        .push(Digest::of(b"new source"));
    assert_ne!(
        derive(&evidence, &worker, &catalog, &implementation)
            .expect("evidence identity")
            .prepared_key,
        original.prepared_key
    );
    assert_ne!(
        derive(
            &request,
            &worker,
            &Digest::of(b"other catalog"),
            &implementation
        )
        .expect("catalog identity")
        .prepared_key,
        original.prepared_key
    );
    assert_ne!(
        derive(
            &request,
            &worker,
            &catalog,
            &Digest::of(b"other implementation")
        )
        .expect("implementation identity")
        .prepared_key,
        original.prepared_key
    );
    assert_eq!(
        request.fixture["accountOverride"],
        json!(null),
        "identity must not mutate caller input"
    );
}

#[test]
fn checkpoint_counters_are_observed_exactly_and_restore_never_replays_prefix() {
    let hash = Digest::of(b"checkpoint");
    let response = json!({"schema":"svm-m10-checkpoint-output/v1","status":"EXECUTED","checkpointSha256":hash,
        "reads":[],"output":{},"metrics":{"prefixTransactionsExecuted":0,"prefixSimulationCalls":0,"prefixCommitCalls":0,"prefixTransactionsReused":3}});
    let observed = checkpoint_response(&response, CheckpointMode::Run, 3)
        .expect("restored checkpoint response");
    assert_eq!(observed.metrics.prefix_transactions_reused, 3);
    for name in [
        "prefixTransactionsExecuted",
        "prefixSimulationCalls",
        "prefixCommitCalls",
    ] {
        let mut poisoned = response.clone();
        poisoned["metrics"][name] = json!(1);
        assert_eq!(
            checkpoint_response(&poisoned, CheckpointMode::Run, 3)
                .expect_err("hidden prefix replay")
                .code,
            "RESTORE_REPLAYED_PREFIX"
        );
    }
    let mut missing = response.clone();
    missing["metrics"]
        .as_object_mut()
        .expect("metrics object")
        .remove("prefixSimulationCalls");
    assert!(checkpoint_response(&missing, CheckpointMode::Run, 3).is_err());
    let mut mismatched = response.clone();
    mismatched["metrics"]["prefixTransactionsReused"] = json!(2);
    assert!(checkpoint_response(&mismatched, CheckpointMode::Run, 3).is_err());
    let mut missing_input = response;
    missing_input["status"] = json!("NEEDS_INPUT");
    assert_eq!(
        checkpoint_response(&missing_input, CheckpointMode::Run, 3)
            .expect_err("guarded missing input")
            .code,
        "NEEDS_INPUT"
    );
}

#[path = "../../_6_fees/tests/support.rs"]
#[cfg(target_os = "linux")]
mod fee_ledger;

#[path = "cache_cases/execution.rs"]
#[cfg(target_os = "linux")]
mod execution;
