use super::_0_implementation::fixture_capabilities;
use serde_json::json;

#[test]
fn recovered_sysvar_requires_explicit_worker_capability() {
    let fixture = json!({"runtime":{"slotHashesData":{}}});
    assert_eq!(
        fixture_capabilities(&fixture, &[]).unwrap_err().code,
        "UNSUPPORTED_RUNTIME_CAPABILITY"
    );
    assert!(fixture_capabilities(&fixture, &["proven-slot-hashes-cache/v1".into()]).is_ok());
    assert!(fixture_capabilities(&json!({"runtime":{}}), &[]).is_ok());
}
