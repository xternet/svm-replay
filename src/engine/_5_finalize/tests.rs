use serde_json::json;
use svm_replay_engine::_5_finalize::{
    ensure_private_directory, inspect_job, write_bytes, write_json,
};

#[test]
fn trace_payload_is_saved_byte_exact_and_never_overwritten() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("trace.json");
    let bytes = br#"[{"lamports":18446744073709551615}]"#;
    assert_eq!(
        write_bytes(&path, bytes).unwrap(),
        svm_replay_protocol::Digest::of(bytes)
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(write_bytes(&path, b"[]").unwrap_err().code, "OUTPUT_IO");
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn interrupted_and_completed_jobs_remain_distinguishable_without_guessing_liveness() {
    let temp = tempfile::tempdir().unwrap();
    assert_eq!(inspect_job(temp.path()).unwrap_err().code, "JOB_NOT_FOUND");
    write_json(
        &temp.path().join("started.json"),
        &json!({"schema":"svm-replay-started/v1","requestId":"a"}),
    )
    .unwrap();
    assert_eq!(inspect_job(temp.path()).unwrap()["outcome"], "INCOMPLETE");
    let receipt = json!({"schema":"svm-replay-receipt/v1","outcome":"CANCELLED"});
    write_json(&temp.path().join("receipt.json"), &receipt).unwrap();
    assert_eq!(inspect_job(temp.path()).unwrap(), receipt);
    assert_eq!(
        write_json(&temp.path().join("receipt.json"), &json!({}))
            .unwrap_err()
            .code,
        "OUTPUT_IO"
    );
    assert_eq!(inspect_job(temp.path()).unwrap(), receipt);
}

#[test]
fn corrupted_receipt_is_an_error_not_incomplete_or_success() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("receipt.json"),
        b"{\"outcome\":\"COMPLETED\",\"outcome\":\"ERROR\"}",
    )
    .unwrap();
    assert!(inspect_job(temp.path()).is_err());
}

#[cfg(unix)]
#[test]
fn newly_created_data_directories_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("data").join("jobs");
    ensure_private_directory(&root).unwrap();
    assert_eq!(
        std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(root.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}
