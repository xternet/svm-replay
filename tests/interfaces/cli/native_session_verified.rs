use crate::unix::inputs;
use serde_json::Value;
use std::{path::PathBuf, time::Duration};
use svm_replay_engine::shared::{
    historical::{SessionCommand, SessionInput, VerifiedSession},
    runtime::{CancellationToken, ExecutionBudget, WorkerLimits, WorkerTransport},
};

pub(super) fn reviewed_input(worker_path: PathBuf) -> SessionInput {
    SessionInput {
        schema: "svm-historical-session-input/v1".into(),
        receipt_path: PathBuf::from(
            std::env::var_os("SVM_SESSION_RECEIPT").expect("reviewed receipt"),
        ),
        receipt_sha256: std::env::var("SVM_SESSION_RECEIPT_SHA256").unwrap(),
        worker_path,
        after: true,
    }
}

#[test]
#[ignore = "requires reviewed SVM_SESSION_RECEIPT/WORKER pins; explicit cached integration driver"]
fn verified_session_binds_control_and_rejects_a_rehashed_false_pass_receipt() {
    let (fixture, worker, owner) = inputs();
    let input = reviewed_input(worker.executable);
    let receipt_path = input.receipt_path.clone();
    let scratch = tempfile::tempdir().unwrap();
    let budget =
        || ExecutionBudget::new(Duration::from_secs(120), CancellationToken::new()).unwrap();
    let transport = || WorkerTransport::new(scratch.path()).with_owner(owner.clone());
    let mut session = VerifiedSession::open(
        input.clone(),
        transport(),
        WorkerLimits::default(),
        budget(),
    )
    .unwrap();
    assert_eq!(session.pins()["slot"], fixture["target"]["targetSlot"]);
    assert_eq!(
        session.pins()["transactionIndex"].as_u64().unwrap(),
        fixture["target"]["index"].as_u64().unwrap() + 1
    );
    assert_eq!(
        session.pins()["initialStateSha256"],
        session.ready()["state"]["stateSha256"]
    );
    assert_eq!(
        session.request(SessionCommand::State, &budget()).unwrap(),
        session.ready()["state"]
    );
    session.close().unwrap();
    let mut corrupt = input.clone();
    corrupt.receipt_sha256 = "0".repeat(64);
    assert_eq!(
        VerifiedSession::open(corrupt, transport(), WorkerLimits::default(), budget())
            .err()
            .unwrap()
            .code,
        "SESSION_INTEGRITY"
    );

    let spoof = tempfile::tempdir().unwrap();
    let mut receipt: Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    for key in ["preparedRequest", "output", "sourcePreparation"] {
        let name = receipt[key]["file"].as_str().unwrap();
        std::fs::copy(
            receipt_path.parent().unwrap().join(name),
            spoof.path().join(name),
        )
        .unwrap();
    }
    let output_path = spoof
        .path()
        .join(receipt["output"]["file"].as_str().unwrap());
    let mut output: Value = serde_json::from_slice(&std::fs::read(&output_path).unwrap()).unwrap();
    output["original"]["logs"][0] = Value::String("tampered recorded log".into());
    let bytes = serde_json::to_vec(&output).unwrap();
    std::fs::write(&output_path, &bytes).unwrap();
    receipt["output"]["sha256"] =
        Value::String(svm_replay_protocol::Digest::of(&bytes).as_str().into());
    receipt["controlEvidence"]["outputSha256"] = receipt["output"]["sha256"].clone();
    let bytes = serde_json::to_vec(&receipt).unwrap();
    let mut spoof_input = input;
    spoof_input.receipt_path = spoof.path().join("receipt.json");
    spoof_input.receipt_sha256 = svm_replay_protocol::Digest::of(&bytes).as_str().into();
    std::fs::write(&spoof_input.receipt_path, bytes).unwrap();
    let error = VerifiedSession::open(spoof_input, transport(), WorkerLimits::default(), budget())
        .err()
        .unwrap();
    assert_eq!(error.code, "SESSION_CONTROL_MISMATCH");
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
}
