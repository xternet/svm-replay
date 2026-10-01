#![cfg(target_os = "linux")]
#[path = "../../../_6_fees/tests/support.rs"]
mod fee_ledger;
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};
use svm_replay_engine::shared::{
    debug::{branch_session, open_session, save_session, SaveOptions},
    runtime::{file_sha256, ProcessOwner},
    trace::{CaptureRequest, ProducerBounds},
};
use svm_replay_protocol::{worker::CaptureWorkerDescriptor, Digest};

#[test]
fn immutable_session_roundtrip_and_branch_preserve_original_control_and_pins() {
    let root = tempfile::tempdir().unwrap();
    let (request, reference) = fee_ledger::fixture(root.path(), "#!/usr/bin/dash\nexit 0\n");
    let mut capture_descriptor = reference.descriptor.clone();
    capture_descriptor.capabilities.extend([
        "runtime-full-capture-cli/v1".into(),
        "runtime-call-journal/v1".into(),
        "runtime-interpreter-debug/v1".into(),
    ]);
    let role = CaptureWorkerDescriptor {
        worker: capture_descriptor,
        reference_sha256: reference.descriptor.sha256.clone(),
        gate_sha256: Digest::of(b"non-executing storage fixture gate"),
    };
    let owner_path = root.path().join("owner-source");
    fs::copy("/usr/bin/true", &owner_path).unwrap();
    let owner = ProcessOwner {
        sha256: file_sha256(&owner_path).unwrap(),
        executable: owner_path,
    };
    let policy=CaptureRequest::parse(&serde_json::to_vec(&json!({"schema":"svm-capture-request/v2","executionMode":"interpreter-debug","level":"calls","sbpfObservations":"none","filter":{"programIds":[],"instructionIndices":[]},"limits":{"maxBytes":1048576,"maxEvents":100,"timeoutMs":10000}})).unwrap()).unwrap();
    let bounds = ProducerBounds {
        register_rows: 0,
        memory_rows: None,
        max_invocations: 64,
    };
    let implementation = Digest::of(b"explicit storage test implementation");
    let options = SaveOptions {
        request: &request,
        reference: &reference.spec,
        reference_descriptor: &reference.descriptor,
        capture: &reference.spec,
        capture_descriptor: &role,
        owner: &owner,
        implementation_sha256: &implementation,
        policy: &policy,
        bounds: &bounds,
        symbols: None,
    };
    let path = root.path().join("session");
    let pin = save_session(&path, &options).unwrap();
    let session = open_session(&path, &pin).unwrap();
    assert_eq!(session.request.fixture, request.fixture);
    assert_eq!(
        session.manifest.files["reference-worker"].sha256,
        reference.descriptor.sha256
    );
    {
        use svm_replay_engine::shared::debug::{open_run, save_run, RunArtifacts};
        let output = json!({"schema":"non-executing-storage-test/v1","inputHash":Digest::of(serde_json::to_vec(&request.fixture).unwrap())});
        let events = vec![json!({"kind":"storage-fixture","notHistoricalEvidence":true})];
        let verification = json!({"status":"PASS","scope":"storage format fixture only"});
        let evidence = json!({"schema":"svm-original-control-evidence/v1","complete":true,"phase":"original-control","fixtureSha256":Digest::of(svm_replay_engine::shared::diff::canonical_json(&request.fixture)),"outputSha256":Digest::of(svm_replay_engine::shared::diff::canonical_json(&output)),"verification":verification});
        let capture_key = svm_replay_engine::shared::trace::capture_identity(
            &Digest::of(svm_replay_engine::shared::diff::canonical_json(
                &request.fixture,
            )),
            &session.manifest.capture.worker,
            &policy,
            &bounds,
        )
        .unwrap();
        let identity = Digest::of(svm_replay_engine::shared::diff::canonical_json(
            &json!({"schema":"svm-m17-debug-identity/v1","captureIdentity":capture_key,"gateSha256":session.manifest.capture.gate_sha256,"referenceSha256":session.manifest.capture.reference_sha256,"implementationSha256":implementation,"phase":"original-control","blockSha256":request.block_sha256,"candidate":request.candidate,"sourceEvidenceHashes":request.source_evidence_hashes}),
        ));
        let journal_events = json!([{"kind":"transaction_begin","accounts":[]},{"kind":"transaction_commit","outcome":{"status":"success"},"accounts":[]}]);
        let journal = json!({"signatures":[request.fixture["target"]["signature"]],"scope":"empty storage-format fixture","before_scope":"empty","before_keys":[],"terminal_scope":"empty","terminal_keys":[],"journal":{"disposition":"COMPLETE","reason":null,"event_count":journal_events.as_array().unwrap().len(),"payload_json":serde_json::to_string(&journal_events).unwrap()}});
        let export=svm_replay_engine::shared::trace::make_export(&policy,&identity,&json!({"schema":"svm-historical-capture-worker-experimental/v1","status":"COMPLETE","execution_mode":"interpreter-debug","captures":[],"journals":[journal]})).unwrap();
        let receipt = json!({"schema":"svm-m17-debug-execution/v1","status":"PASS","executionMode":"interpreter-debug","referenceSha256":session.manifest.reference.sha256,"captureWorkerSha256":session.manifest.capture.worker.sha256,"gateSha256":session.manifest.capture.gate_sha256,"implementationSha256":implementation,"eventCount":events.len(),"eventsSha256":Digest::of(svm_replay_engine::shared::diff::canonical_json(&json!(events))),"exports":[{"phase":"original-control","artifact":export.artifact}]});
        let exports = vec![("original-control".to_owned(), export)];
        let run = root.path().join("run");
        let artifacts = RunArtifacts {
            output: &output,
            verification: &verification,
            control_evidence: &evidence,
            events: &events,
            receipt: &receipt,
            exports: &exports,
        };
        let run_pin = save_run(&run, &session, &artifacts).unwrap();
        let loaded = open_run(&run, &run_pin, &pin).unwrap();
        assert_eq!(loaded.output, output);
        assert_eq!(loaded.events, events);
        assert!(open_run(&run, &Digest::of(b"wrong run pin"), &pin).is_err());
        let target = run.join("events.json");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(target, b"[]").unwrap();
        assert!(open_run(&run, &run_pin, &pin).is_err());
    }
    assert!(
        save_session(&path, &options).is_err(),
        "create-only store never overwrites"
    );
    let branch = root.path().join("branch");
    let variant =
        json!({"replacementTransactionBase64":request.fixture["target"]["transactionBase64"]});
    let branch_pin = branch_session(&path, &pin, &branch, &variant).unwrap();
    let derived = open_session(&branch, &branch_pin).unwrap();
    assert_eq!(derived.control.fixture, request.fixture);
    assert_eq!(
        derived.request.fixture["target"]["replacementTransactionBase64"],
        variant["replacementTransactionBase64"]
    );
    assert_ne!(
        derived.manifest.session_identity,
        session.manifest.session_identity
    );
    let grandchild = root.path().join("grandchild");
    let overrides = json!({"requestedAccountOverrides":[{"pubkey":request.fixture["accounts"][0]["pubkey"],"lamports":"1001"}]});
    let grandpin = branch_session(&branch, &branch_pin, &grandchild, &overrides).unwrap();
    let grand = open_session(&grandchild, &grandpin).unwrap();
    assert_eq!(
        grand.manifest.files["control-request.json"].sha256,
        session.manifest.files["control-request.json"].sha256
    );
    assert!(branch_session(
        &path,
        &pin,
        &root.path().join("invalid"),
        &json!({"targetSlot":99})
    )
    .is_err());
    let worker = branch.join("capture-worker");
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&worker, b"changed").unwrap();
    assert!(
        open_session(&branch, &branch_pin).is_err(),
        "corrupt worker cannot be reused"
    );
    assert!(Path::new(&session.directory).is_absolute());
}

#[test]
fn session_does_not_trust_a_rehashed_foreign_manifest_or_symlink() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("missing");
    let pin = svm_replay_engine::shared::debug::SessionPin {
        manifest_sha256: Digest::of(b"not a saved session"),
        worker_sha256: Digest::of(b"other worker"),
    };
    fs::create_dir(&path).unwrap();
    fs::write(path.join("manifest.json"), b"not a saved session").unwrap();
    assert!(open_session(&path, &pin).is_err());
    symlink(&path, root.path().join("link")).unwrap();
    assert!(open_session(&root.path().join("link"), &pin).is_err());
}
