use super::_0_implementation::fee_evidence;
use serde_json::{json, Value};

#[test]
fn fee_evidence_requires_a_recorded_charge_and_lookup_addresses() {
    let legacy = json!({"meta":{"fee":42},"transaction":{"message":{}}});
    assert_eq!(
        fee_evidence(&legacy).unwrap(),
        (42, json!({"writable":[],"readonly":[]}))
    );
    let mut looked_up = legacy.clone();
    looked_up["transaction"]["message"]["addressTableLookups"] = json!([{"accountKey":"table"}]);
    assert!(fee_evidence(&looked_up).is_err());
    let loaded = json!({"writable":["writable"],"readonly":["readonly"]});
    looked_up["meta"]["loadedAddresses"] = loaded.clone();
    assert_eq!(fee_evidence(&looked_up).unwrap(), (42, loaded));
    for invalid in [Value::Null, json!(-1), json!("42")] {
        looked_up["meta"]["fee"] = invalid;
        assert!(fee_evidence(&looked_up).is_err());
    }
}
