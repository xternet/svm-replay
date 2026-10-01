use super::*;

#[test]
fn controller_disconnect_is_cancelled_without_publishing_a_result() {
    let job = tempfile::tempdir().unwrap();
    let result = finish_job(
        Err(Error::new("CANCELLED", "debug controller disconnected")),
        json!({"controlVerification":null,"verification":null}),
        job.path(),
        Instant::now(),
    )
    .unwrap();
    assert_eq!(result["outcome"], "CANCELLED");
    assert_eq!(result["error"]["code"], "CANCELLED");
    assert!(result.get("output").is_none());
    assert!(!job.path().join("result.json").exists());
}

#[test]
fn provider_deadline_is_timeout_not_missing_history_or_generic_error() {
    let job = tempfile::tempdir().unwrap();
    let result = finish_job(
        Err(Error::new("SOURCE_DEADLINE", "bounded request expired")),
        json!({"controlVerification":null,"verification":null}),
        job.path(),
        Instant::now(),
    )
    .unwrap();
    assert_eq!(result["outcome"], "TIMEOUT");
    assert_eq!(result["error"]["code"], "SOURCE_DEADLINE");
    assert!(result.get("output").is_none());
    assert!(!job.path().join("result.json").exists());
}

#[test]
fn requested_failure_keeps_control_evidence_without_a_result() {
    let job = tempfile::tempdir().unwrap();
    let verification = json!({"status":"PASS","scope":"protocol-lifecycle-test"});
    let proof =
        crate::_4_simulate::control::evidence(&json!({}), &json!({"computed":true}), &verification);
    let error = crate::_4_simulate::control::attach(
        Error::new("WORKER_TIMEOUT", "requested failure"),
        Some(&proof),
    );
    let result = finish_job(
        Err(error),
        json!({"controlVerification":null,"verification":null}),
        job.path(),
        Instant::now(),
    )
    .unwrap();
    assert_eq!(result["outcome"], "TIMEOUT");
    assert_eq!(result["controlVerification"], verification);
    assert_eq!(result["controlEvidence"], proof);
    assert_eq!(result["incomplete"]["originalControlVerified"], true);
    assert_eq!(result["incomplete"]["requestedReplayVerified"], false);
    assert!(result["verification"].is_null());
    assert!(result.get("output").is_none());
    assert!(!job.path().join("result.json").exists());
}

#[test]
fn incomplete_account_preserves_available_evidence_not_a_result() {
    let job = tempfile::tempdir().unwrap();
    let artifact = json!({"file":"source-observations.json","sha256":Digest::of(b"[]")});
    let error = Error::new("UNSUPPORTED_HISTORICAL_ACCOUNT", "exact image absent")
        .with_details(json!({"pubkey":"account","parentSlot":123,"required":"owner and bytes"}));
    let receipt = finish_job(
        Err(error),
        json!({"sourceObservations":artifact}),
        job.path(),
        Instant::now(),
    )
    .unwrap();
    assert_eq!(receipt["outcome"], "UNSUPPORTED");
    assert_eq!(
        receipt["incomplete"]["category"],
        "HISTORICAL_ACCOUNT_MISSING"
    );
    assert_eq!(receipt["incomplete"]["input"]["slot"], 123);
    assert_eq!(
        receipt["incomplete"]["available"]["sourceObservations"],
        artifact
    );
    assert_eq!(receipt["incomplete"]["requestedReplayVerified"], false);
    assert_eq!(receipt["incomplete"]["estimated"], false);
    assert!(receipt.get("output").is_none());
}

#[test]
fn incomplete_categories_use_structured_evidence_and_keep_unknowns() {
    let cases = [
        (
            "UNSUPPORTED_REQUIRED_ACCOUNT",
            json!({"discovery":{"pubkey":"SysvarS1otHashes111111111111111111111111111"}}),
            "SLOT_HASHES_MISSING",
        ),
        (
            "UNSUPPORTED_HISTORICAL_EPOCH_STAKE",
            json!({}),
            "BANK_CONTEXT_MISSING",
        ),
        ("UNSUPPORTED_RUNTIME", json!({}), "RUNTIME_UNSUPPORTED"),
        (
            "UNSUPPORTED_TRANSACTION_VERSION",
            json!({"version": 2}),
            "RUNTIME_UNSUPPORTED",
        ),
        (
            "CAPABILITY_UNAVAILABLE",
            json!({}),
            "CAPABILITY_UNSUPPORTED",
        ),
        ("UNSUPPORTED_RESOURCE_LIMIT", json!({}), "RESOURCE_LIMIT"),
        ("SOURCE_RATE_LIMIT", json!({}), "PROVIDER_ERROR"),
        (
            "SOURCE_INTEGRITY",
            json!({"pubkey":"SysvarS1otHashes111111111111111111111111111"}),
            "UNCLASSIFIED",
        ),
        ("UNSUPPORTED_NEW_THING", json!({}), "UNCLASSIFIED"),
    ];
    for (code, details, expected) in cases {
        let job = tempfile::tempdir().unwrap();
        let receipt = finish_job(
            Err(Error::new(code, "SlotHashes in message is not evidence").with_details(details)),
            json!({}),
            job.path(),
            Instant::now(),
        )
        .unwrap();
        assert_eq!(receipt["incomplete"]["category"], expected, "{code}");
        assert_eq!(receipt["error"]["code"], code);
    }
}

#[test]
fn successful_result_has_no_incomplete_envelope() {
    let job = tempfile::tempdir().unwrap();
    let receipt = finish_job(
        Ok(json!({"original":{"status":"ok"}})),
        json!({}),
        job.path(),
        Instant::now(),
    )
    .unwrap();
    assert_eq!(receipt["outcome"], "COMPLETED");
    assert!(receipt.get("incomplete").is_none());
}
