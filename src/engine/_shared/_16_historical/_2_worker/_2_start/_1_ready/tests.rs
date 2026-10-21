use super::*;
use serde_json::json;

#[test]
fn initialized_ready_is_explicitly_control_free_not_a_false_original_pass() {
    let ready = json!({"schema":"svm-native-session/v1","kind":"ready",
        "state":{"slot":10,"transactionIndex":0},"original":null,
        "boundaryKind":"initialized-slot","originalControl":{"status":"NOT_APPLICABLE"}});
    assert!(validate(&ready, true, 10).is_ok());
    for (name, value) in [
        ("original", json!({"status":"ok"})),
        ("originalControl", json!({"status":"PASS"})),
        ("boundaryKind", json!("transaction")),
        ("state", json!({"slot":10,"transactionIndex":1})),
        ("state", json!({"slot":11,"transactionIndex":0})),
    ] {
        let mut invalid = ready.clone();
        invalid[name] = value;
        assert!(validate(&invalid, true, 10).is_err(), "{invalid}");
    }
    assert!(validate(&ready, false, 10).is_err());
}

#[test]
fn legacy_transaction_ready_still_requires_a_real_original_control() {
    let mut ready = json!({"schema":"svm-native-session/v1","kind":"ready",
        "state":{"slot":10,"transactionIndex":5},"original":{"status":"ok"}});
    assert!(validate(&ready, false, 10).is_ok());
    ready["original"] = Value::Null;
    assert!(validate(&ready, false, 10).is_err());
}
