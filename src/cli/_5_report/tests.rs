use super::*;
use serde_json::json;

#[test]
fn internal_error_report_contains_only_safe_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.md");
    let result = json!({"outcome":"ERROR","error":{"code":"INTERNAL",
        "message":"secret-key /private/path"},"payload":"private payload"});
    assert!(write_if_internal(&path, &result).unwrap());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("INTERNAL"));
    assert!(text.contains(ISSUE_URL));
    assert!(!text.contains("secret-key"));
    assert!(!text.contains("/private/path"));
    assert!(!text.contains("private payload"));
    assert!(write_if_internal(&path, &result).is_err());
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}

#[test]
fn expected_failures_never_create_reports() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.md");
    for outcome in [
        "COMPLETED",
        "UNSUPPORTED",
        "NEEDS_INPUT",
        "MISMATCH",
        "TIMEOUT",
        "CANCELLED",
    ] {
        assert!(!write_if_internal(
            &path,
            &json!({"outcome":outcome,
            "error":{"code":"INTERNAL"}})
        )
        .unwrap());
    }
    for code in ["INVALID_REQUEST", "SOURCE_IO", "INTERNAL-secret", ""] {
        assert!(!write_if_internal(
            &path,
            &json!({"outcome":"ERROR",
            "error":{"code":code}})
        )
        .unwrap());
    }
    assert!(!path.exists());
}

#[test]
fn report_write_failures_are_returned() {
    let dir = tempfile::tempdir().unwrap();
    assert!(write_if_internal(
        &dir.path().join("absent/report.md"),
        &json!({"outcome":"ERROR","error":{"code":"INTERNAL"}})
    )
    .is_err());
}
