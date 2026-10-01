use super::super::TraceOptions;
use serde_json::json;

#[test]
fn complete_collection_never_accepts_partial_or_missing_exports() {
    let mut options: TraceOptions = serde_json::from_value(json!({
        "capture":{"schema":"svm-capture-request/v2","executionMode":"jit",
            "level":"calls","sbpfObservations":"none",
            "filter":{"programIds":[],"instructionIndices":[]},
            "limits":{"maxBytes":1024,"maxEvents":100,"timeoutMs":1000}},
        "bounds":{"registerRows":0,"memoryRows":null,"maxInvocations":64},
        "require_complete":true
    }))
    .unwrap();
    assert!(options
        .check_completeness(&[json!({"artifact":{"status":"COMPLETE"}})])
        .is_ok());
    assert_eq!(
        options.check_completeness(&[]).unwrap_err().code,
        "COLLECTION_INCOMPLETE"
    );
    for status in ["TRUNCATED", "ERROR", "PARTIAL", "UNKNOWN"] {
        let exports = [json!({"artifact":{"status":status}})];
        assert_eq!(
            options.check_completeness(&exports).unwrap_err().code,
            "COLLECTION_INCOMPLETE"
        );
    }
    options.require_complete = false;
    assert!(options
        .check_completeness(&[json!({"artifact":{"status":"TRUNCATED"}})])
        .is_ok());
}
