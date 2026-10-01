use super::*;
#[test]
fn inline_and_absolute_file_json_match_and_bad_inputs_reject() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("overrides.json");
    let patch = r#"[{"pubkey":"11111111111111111111111111111111","lamports":"10"}]"#;
    std::fs::write(&file, patch).unwrap();
    assert_eq!(
        overrides(patch).unwrap(),
        overrides(&format!("@{}", file.display())).unwrap()
    );
    assert!(overrides("{}").is_err());
    assert!(overrides("[]").is_err());
    assert!(overrides("@").is_err());
    assert!(replacement(r#"{"transactionBase64":"invalid"}"#).is_err());
    assert!(replacement(r#"{"transactionBase64":"","extra":1}"#).is_err());
    let trace = collect(r#"{"calls":true}"#).unwrap().unwrap();
    assert_eq!(
        trace.capture.execution_mode,
        if cfg!(all(target_arch = "x86_64", not(windows))) {
            "jit"
        } else {
            "interpreter"
        }
    );
}
