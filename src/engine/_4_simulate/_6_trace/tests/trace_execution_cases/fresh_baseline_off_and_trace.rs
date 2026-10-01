use super::*;

#[test]
fn fresh_baseline_off_and_trace_are_gated_for_each_control_and_requested_phase() {
    let root = tempfile::tempdir().unwrap();
    let (mut request, reference, capture) = fixture(root.path());
    request.fixture["target"]["replacementTransactionBase64"] =
        request.fixture["target"]["transactionBase64"].clone();
    let before = request.fixture.clone();
    let executed = run(root.path(), &request, &reference, &capture).unwrap();
    assert_eq!(executed.receipt["status"], "PASS");
    assert_eq!(executed.receipt["workerCalls"], 6);
    assert_eq!(executed.exports.len(), 2);
    assert_eq!(executed.exports[0].1.artifact["status"], "COMPLETE");
    assert_eq!(request.fixture, before);
    assert_eq!(
        fs::read_to_string(root.path().join("calls"))
            .unwrap()
            .lines()
            .count(),
        6
    );
}

#[test]
fn gate_mismatch_and_trace_off_drift_cannot_publish_export() {
    let root = tempfile::tempdir().unwrap();
    let (request, reference, mut capture) = fixture(root.path());
    capture.reference_sha256 = Digest::of(b"wrong reference");
    assert_eq!(
        run(root.path(), &request, &reference, &capture)
            .err()
            .unwrap()
            .code,
        "WORKER_IDENTITY"
    );
    assert!(!root.path().join("calls").exists());
    capture.reference_sha256 = reference.descriptor.sha256.clone();
    fs::write(root.path().join("corrupt-off"), b"explicit protocol fault").unwrap();
    assert_eq!(
        run(root.path(), &request, &reference, &capture)
            .err()
            .unwrap()
            .code,
        "TRACE_PARITY_MISMATCH"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("calls"))
            .unwrap()
            .lines()
            .count(),
        2
    );
}

#[test]
fn capture_options_mutation_is_not_accepted_under_original_identity() {
    let root = tempfile::tempdir().unwrap();
    let (request, reference, capture) = fixture(root.path());
    fs::write(
        root.path().join("mutate-options"),
        b"explicit options mutation fault",
    )
    .unwrap();
    let error = run(root.path(), &request, &reference, &capture)
        .err()
        .expect("mutated options must fail");
    assert_eq!(error.code, "TRACE_OPTIONS_INTEGRITY");
    assert_eq!(error.details.unwrap()["partialExportPublished"], false);
}

#[test]
fn requested_failure_retains_control_proof_without_publishing_trace() {
    let root = tempfile::tempdir().unwrap();
    let (mut request, reference, capture) = fixture(root.path());
    let original = run(root.path(), &request, &reference, &capture).unwrap();
    request.fixture["target"]["replacementTransactionBase64"] =
        request.fixture["target"]["transactionBase64"].clone();
    fs::write(
        root.path().join("fail-requested"),
        b"explicit requested failure",
    )
    .unwrap();
    let error = run(root.path(), &request, &reference, &capture)
        .err()
        .expect("requested failure");
    assert_eq!(error.code, "NEEDS_INPUT");
    let details = error.details.unwrap();
    assert_eq!(
        details["controlEvidence"]["verification"],
        original.control_verification
    );
    assert_eq!(
        details["controlEvidence"]["outputSha256"],
        json!(Digest::of(svm_replay_engine::shared::diff::canonical_json(
            &original.output
        )))
    );
    assert_eq!(details["controlEvidence"]["complete"], true);
    assert_eq!(details["partialExportPublished"], false);
    assert!(details["controlEvidence"].get("output").is_none());
    assert_eq!(details["cause"]["status"], "NEEDS_INPUT");
}
