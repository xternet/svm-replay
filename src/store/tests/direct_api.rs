use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;
use svm_replay_store::{parse_request, Store};

#[test]
fn rust_api_reopens_persisted_bytes_without_a_cli() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut store = Store::open(dir.path()).expect("open store directly");
    let request = parse_request(
        br#"{"version":1,"op":"put","namespace":"prepared","key":"rust-api","dataBase64":"cnVzdCBieXRlcw=="}"#,
    )
    .expect("parse versioned request");
    let stored = store.execute(request).expect("put directly");
    assert_eq!(stored["status"], "STORED");
    drop(store);

    let mut reopened = Store::open(dir.path()).expect("reopen directly");
    let hit = reopened
        .execute(json!({"version":1,"op":"get","namespace":"prepared","key":"rust-api"}))
        .expect("get directly");
    assert_eq!(hit["status"], "HIT");
    assert_eq!(hit["sha256"], stored["sha256"]);
    assert_eq!(
        STANDARD
            .decode(hit["dataBase64"].as_str().expect("base64 string"))
            .expect("decode stored bytes"),
        b"rust bytes"
    );
    let conflict = reopened
        .execute(json!({"version":1,"op":"put","namespace":"prepared","key":"rust-api","dataBase64":STANDARD.encode(b"different bytes")}))
        .expect_err("immutable key rejects different bytes");
    assert_eq!(conflict.code, "CONFLICT");
    assert!(!conflict.retryable);
}

#[test]
fn rust_api_error_retains_committed_observed_budget_details() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut store = Store::open(dir.path()).expect("open store directly");
    let limits = json!({"reads":2,"requests":3,"bytes":5});
    store
        .execute(
            json!({"version":1,"op":"budget","action":"open","id":"direct-job","limits":limits}),
        )
        .expect("open durable budget");
    let observed = json!({"reads":1,"requests":1,"bytes":7});
    let error = store
        .execute(json!({"version":1,"op":"budget","action":"observe","id":"direct-job","amount":observed}))
        .expect_err("actual overrun must be reported");
    assert_eq!(error.code, "BUDGET_EXCEEDED");
    assert_eq!(
        error.details,
        Some(json!({"used":observed,"limits":limits}))
    );
    assert_eq!(error.response()["error"]["code"], "BUDGET_EXCEEDED");
    drop(store);

    let inspected = Store::open(dir.path())
        .expect("reopen budget store")
        .execute(json!({"version":1,"op":"budget","action":"inspect","id":"direct-job"}))
        .expect("inspect committed spending");
    assert_eq!(inspected["used"], observed);
    assert_eq!(inspected["limits"], limits);
}
