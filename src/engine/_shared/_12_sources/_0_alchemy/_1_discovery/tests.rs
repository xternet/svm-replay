use super::*;
#[test]
fn lookup_requires_exact_identity_and_slot() {
    let sig = "1".repeat(64);
    let response = |result: Value| {
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"result":result})).unwrap()
    };
    assert_eq!(
        transaction_slot(
            &response(json!({"slot":42,"transaction":{"signatures":[sig]}})),
            &sig
        )
        .unwrap(),
        42
    );
    for value in [
        Value::Null,
        json!({"slot":42,"transaction":{"signatures":["different"]}}),
        json!({"slot":-1,"transaction":{"signatures":[sig]}}),
    ] {
        assert!(transaction_slot(&response(value), &sig).is_err());
    }
}
