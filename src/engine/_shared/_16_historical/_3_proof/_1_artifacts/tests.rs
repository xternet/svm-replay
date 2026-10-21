use super::*;
use crate::shared::runtime::CancellationToken;
use std::time::Duration;

fn budget() -> ExecutionBudget {
    ExecutionBudget::new(Duration::from_secs(10), CancellationToken::new()).unwrap()
}

#[test]
fn immutable_artifact_pin_path_bounds_and_cancellation_are_enforced() {
    let root = tempfile::tempdir().unwrap();
    let raw = br#"{"actual":"bytes"}"#;
    let path = root.path().join("artifact.json");
    std::fs::write(&path, raw).unwrap();
    let pin = json!({"file":"artifact.json","sha256":Digest::of(raw).as_str()});
    let call = budget();
    assert_eq!(
        _1_artifacts::artifact(root.path(), &pin, 1024, &call).unwrap(),
        raw
    );
    for name in ["../artifact.json", "/artifact.json", ""] {
        let mut bad = pin.clone();
        bad["file"] = json!(name);
        assert!(_1_artifacts::artifact(root.path(), &bad, 1024, &call).is_err());
    }
    assert!(_1_artifacts::artifact(root.path(), &pin, 2, &call).is_err());
    let mut bad = pin.clone();
    bad["sha256"] = json!("0".repeat(64));
    assert_eq!(
        _1_artifacts::artifact(root.path(), &bad, 1024, &call)
            .unwrap_err()
            .code,
        "SESSION_INTEGRITY"
    );
    call.cancel();
    assert_eq!(
        _1_artifacts::artifact(root.path(), &pin, 1024, &call)
            .unwrap_err()
            .code,
        "WORKER_CANCELLED"
    );
}

#[cfg(unix)]
#[test]
fn symlink_to_unrelated_artifact_is_rejected_before_read() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = outside.path().join("data.json");
    std::fs::write(&path, b"{}\n").unwrap();
    std::os::unix::fs::symlink(&path, root.path().join("escape")).unwrap();
    assert!(_1_artifacts::artifact(
        root.path(),
        &json!({"file":"escape","sha256":Digest::of(b"{}\n").as_str()}),
        1024,
        &budget()
    )
    .is_err());
}
