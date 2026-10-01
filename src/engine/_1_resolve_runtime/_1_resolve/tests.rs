use super::_0_implementation::data_only_capability;
use serde_json::json;

#[test]
fn recovered_sysvar_requires_explicit_worker_capability() {
    let fixture = json!({"runtime":{"slotHashesData":{}}});
    assert_eq!(
        data_only_capability(&fixture, &[]).unwrap_err().code,
        "UNSUPPORTED_RUNTIME_CAPABILITY"
    );
    assert!(data_only_capability(&fixture, &["proven-slot-hashes-cache/v1".into()]).is_ok());
    assert!(data_only_capability(&json!({"runtime":{}}), &[]).is_ok());
}
