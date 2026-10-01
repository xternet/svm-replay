use serde_json::json;
use svm_replay_protocol::{parse_json, validate_safe_numbers, Digest, Limits};

#[test]
fn duplicate_keys_are_rejected_at_any_depth() {
    for raw in [r#"{"a":1,"a":2}"#, r#"{"a":[{"x":1,"x":2}]}"#] {
        assert!(parse_json(raw.as_bytes()).is_err());
    }
}

#[test]
fn archives_keep_large_integers_but_public_numbers_are_safe() {
    let raw = br#"{"amount":18446744073709551615}"#;
    let parsed = parse_json(raw).unwrap();
    assert_eq!(parsed["amount"].to_string(), "18446744073709551615");
    assert!(validate_safe_numbers(&parsed).is_err());
    validate_safe_numbers(&json!({"amount":"18446744073709551615","index":12})).unwrap();
}

#[test]
fn digest_requires_canonical_lowercase_hex() {
    assert!(Digest::new("z".repeat(64)).is_err());
    assert!(Digest::new("A".repeat(64)).is_err());
    assert_eq!(
        Digest::of(b"abc").as_str(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn request_limits_reject_zero_excess_and_unknown_fields() {
    let valid = json!({"timeoutMs":1000,"maxOutputBytes":1024,"maxDiagnosticBytes":512});
    let limits: Limits = serde_json::from_value(valid.clone()).unwrap();
    limits.validate().unwrap();
    let mut zero = valid.clone();
    zero["timeoutMs"] = json!(0);
    assert!(serde_json::from_value::<Limits>(zero)
        .unwrap()
        .validate()
        .is_err());
    let mut extra = valid;
    extra["silentFallback"] = json!(true);
    assert!(serde_json::from_value::<Limits>(extra).is_err());
}

#[test]
fn extended_cold_acquisition_deadline_remains_explicit_and_bounded() {
    let mut limits = Limits {
        timeout_ms: 900_000,
        max_output_bytes: 1024,
        max_diagnostic_bytes: 512,
    };
    limits.validate().unwrap();
    for timeout in [900_001, 1_800_000, 3_600_000] {
        limits.timeout_ms = timeout;
        limits.validate().unwrap();
    }
    limits.timeout_ms = 3_600_001;
    assert!(limits.validate().is_err());
}
