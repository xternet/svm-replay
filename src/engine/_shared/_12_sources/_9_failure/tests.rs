use super::*;

fn identity(kind: &str, id: &str) -> Value {
    json!({"kind":kind,"id":id})
}

#[test]
fn unavailable_data_keeps_details_and_is_not_duplicated() {
    let provider = identity("alchemy-historical", "archive-a");
    let error = SourceError::new("SOURCE_UNAVAILABLE", "historical block unavailable")
        .details(json!({"slot":42}));
    let error = attribute_source_failure(error, &provider);
    assert_eq!(error.code, "SOURCE_UNAVAILABLE");
    assert_eq!(error.details.as_ref().unwrap()["slot"], 42);
    assert!(error
        .message
        .starts_with("Missing historical data from provider: Alchemy."));
    let again = attribute_source_failure(error.clone(), &provider);
    assert_eq!(again.message, error.message);
    assert_eq!(again.details, error.details);
}

#[test]
fn connection_failures_are_not_mislabeled_as_missing_data() {
    for code in [
        "SOURCE_RATE_LIMIT",
        "SOURCE_HTTP_ERROR",
        "SOURCE_RPC_ERROR",
        "SOURCE_TRANSPORT",
        "SOURCE_DEADLINE",
    ] {
        let error = attribute_source_failure(
            SourceError::new(code, "original reason"),
            &identity("alchemy-historical", "archive"),
        );
        assert_eq!(error.code, code);
        assert!(error
            .message
            .starts_with("Provider request failed: Alchemy."));
        assert_eq!(
            error.details.unwrap()["providerFailure"]["kind"],
            "PROVIDER_REQUEST_FAILED"
        );
    }
}

#[test]
fn engine_validation_runtime_budget_and_local_errors_are_not_blame_shifted() {
    for code in [
        "SOURCE_INTEGRITY",
        "SOURCE_CACHE_ERROR",
        "SOURCE_CONTEXT_MISMATCH",
        "SOURCE_CONFIGURATION",
        "SOURCE_RESOURCE_LIMIT",
        "UNSUPPORTED_RUNTIME",
        "MISMATCH",
        "INVALID_QUERY",
        "NEEDS_INPUT",
        "CANCELLED",
    ] {
        let original = SourceError::new(code, "original reason");
        let error =
            attribute_source_failure(original.clone(), &identity("alchemy-historical", "archive"));
        assert_eq!(error.message, original.message);
        assert!(error.details.is_none());
    }
    for kind in ["captured-history", "supplied-bank"] {
        let error = attribute_source_failure(
            SourceError::new("SOURCE_UNAVAILABLE", "missing local input"),
            &identity(kind, "local"),
        );
        assert_eq!(error.message, "missing local input");
    }
}

#[test]
fn provider_names_are_from_identities_not_assumed_to_be_alchemy() {
    let error = attribute_source_failure(
        SourceError::new("SOURCE_UNAVAILABLE", "absent"),
        &identity("future-historical", "archive-b"),
    );
    assert!(error.message.contains("provider: archive-b."));
    assert!(!error.message.contains("Alchemy"));
}

#[test]
fn unproven_absence_is_not_assumed_to_be_missing_provider_data() {
    let error = SourceError::new(
        "UNSUPPORTED_HISTORICAL_ACCOUNT",
        "cannot prove account absence",
    )
    .details(json!({"reason":"unproven-account-absence"}));
    let result =
        attribute_source_failure(error.clone(), &identity("alchemy-historical", "archive"));
    assert_eq!(result.message, error.message);
    assert_eq!(result.details, error.details);
}
