use super::*;
use serde_json::json;

#[test]
fn coordinator_pin_rejects_replacement_before_result_publication() {
    let root = tempfile::tempdir().unwrap();
    let executable = root.path().join("explicit test coordinator image");
    std::fs::write(&executable, b"first test image").unwrap();
    let pin = CoordinatorPin::from_path(executable.clone()).unwrap();
    assert_eq!(pin.sha256, Digest::of(b"first test image"));
    pin.verify().unwrap();
    std::fs::write(&executable, b"replacement test image").unwrap();
    let second = CoordinatorPin::from_path(executable.clone()).unwrap();
    assert_ne!(pin.sha256, second.sha256);
    let result = pin.check_result(Ok(json!({"notPublished":true})));
    assert_eq!(result.unwrap_err().code, "COORDINATOR_IDENTITY");
    let receipt = crate::_5_finalize::job::finish_job(
        pin.check_result(Ok(json!({"notPublished":true}))),
        json!({"controlVerification":null,"verification":null}),
        root.path(),
        std::time::Instant::now(),
    )
    .unwrap();
    assert_eq!(receipt["outcome"], "ERROR");
    assert!(!root.path().join("result.json").exists());
    let proof = json!({"phase":"original-control","verification":{"status":"PASS"},"complete":true,"outputSha256":Digest::of(b"earlier verified test output")});
    let primary = Error::new("NEEDS_INPUT", "earlier exact-input failure")
        .with_details(json!({"controlEvidence":proof}));
    let error = pin.check_result(Err(primary)).unwrap_err();
    let details = error.details.unwrap();
    assert_eq!(details["primaryError"]["code"], "NEEDS_INPUT");
    assert_eq!(details["controlEvidence"], proof);
    std::fs::remove_file(executable).unwrap();
    assert_eq!(pin.verify().unwrap_err().code, "COORDINATOR_IDENTITY");
}

#[test]
fn changed_coordinator_cannot_commit_cache_or_complete_debugger() {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let root = tempfile::tempdir().unwrap();
    let executable = root.path().join("explicit publication test image");
    std::fs::write(&executable, b"original image").unwrap();
    let pin = CoordinatorPin::from_path(executable.clone()).unwrap();
    assert_eq!(
        pin.verify_identity(&Digest::of(b"different implementation"))
            .unwrap_err()
            .code,
        "COORDINATOR_IDENTITY"
    );
    let mut store = svm_replay_store::Store::open(&root.path().join("cache")).unwrap();
    let key = Digest::of(b"publication test cache key");
    std::fs::write(&executable, b"changed image").unwrap();
    for namespace in ["prepared", "result"] {
        let error = publish(Some(&pin), || store.execute(json!({"version":1,"op":"put","namespace":namespace,"key":key,"dataBase64":STANDARD.encode(b"verified test payload")})).map_err(|e|Error::new(e.code,e.message))).unwrap_err();
        assert_eq!(error.code, "COORDINATOR_IDENTITY");
        let read = store
            .execute(json!({"version":1,"op":"get","namespace":namespace,"key":key}))
            .unwrap();
        assert_eq!(read["status"], "MISS");
    }
    let (controller, mut driver) = crate::shared::debug::channel();
    let error = publish(Some(&pin), || driver.complete(&json!({"status":"PASS"}))).unwrap_err();
    assert_eq!(error.code, "COORDINATOR_IDENTITY");
    assert_eq!(controller.try_next_event().unwrap(), None);
}
