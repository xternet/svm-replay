use super::*;
#[test]
fn saving_incomplete_output_keeps_category_and_never_adds_result() {
    let directory = tempfile::tempdir().unwrap();
    let receipt = json!({"outcome":"UNSUPPORTED","error":{"code":"UNSUPPORTED_HISTORICAL_ACCOUNT"},
        "incomplete":{"category":"HISTORICAL_ACCOUNT_MISSING","estimated":false,"available":{}}});
    let summary = finish(receipt.clone(), "sig", &[], Some(directory.path())).unwrap();
    assert_eq!(summary["incomplete"], receipt["incomplete"]);
    let saved: Value =
        serde_json::from_slice(&std::fs::read(summary["savedTo"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(saved, receipt);
    assert!(summary.get("result").is_none());
}
#[test]
fn selecting_status_does_not_load_trace_payloads() {
    let root = tempfile::tempdir().unwrap();
    let bytes = br#"{"original":{"status":"ok","logs":[]}}"#;
    std::fs::write(root.path().join("result.json"), bytes).unwrap();
    let receipt = json!({"outcome":"COMPLETED", "receiptPath":root.path().join("receipt.json"),
        "output":{"file":"result.json","sha256":Digest::of(bytes)},
        "trace":{"exports":[{"file":"trace-original-control.json","sha256":Digest::of(b"missing")}]}});
    let selected = finish(receipt, "sig", &["status".into()], None).unwrap();
    assert!(selected["trace"]["exports"][0].get("data").is_none());
    assert_eq!(selected["result"]["original"]["status"], "ok");
}
#[test]
fn default_materializes_results_and_filter_keeps_verification() {
    let root = tempfile::tempdir().unwrap();
    let bytes = br#"{"original":{"status":"ok","logs":[],"computeUnits":150},"requestedOverrides":{"status":"ok","logs":[],"computeUnits":150}}"#;
    std::fs::write(root.path().join("result.json"), bytes).unwrap();
    let receipt = json!({"outcome":"COMPLETED", "receiptPath":root.path().join("receipt.json"),
        "controlVerification":{"status":"PASS"},"output":{"file":"result.json","sha256":Digest::of(bytes)}});
    let full = finish(receipt.clone(), "sig", &[], None).unwrap();
    assert_eq!(full["result"]["original"]["computeUnits"], 150);
    let selected = finish(receipt.clone(), "sig", &["status".into()], None).unwrap();
    assert!(selected["result"]["original"].get("logs").is_none());
    assert!(selected["result"]["requestedOverrides"]
        .get("logs")
        .is_none());
    assert_eq!(selected["controlVerification"]["status"], "PASS");
    std::fs::write(root.path().join("result.json"), b"{}").unwrap();
    assert_eq!(
        finish(receipt, "sig", &[], None).unwrap_err().code,
        "OUTPUT_INTEGRITY"
    );
    assert!(validate_fields(&["invented".into()]).is_err());
}
