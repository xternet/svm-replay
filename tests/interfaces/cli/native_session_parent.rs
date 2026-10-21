use std::process::Command;

#[test]
fn parent_lifetime_binding_rejects_an_unrelated_pid_before_reading_inputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .args(["session", "--parent-pid", "1", "--request", "missing.json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["error"]["code"], "SESSION_PARENT");
}

#[test]
fn slot_session_reaches_explicit_provider_validation_without_fetching_history() {
    let output = Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .args([
            "session",
            "--after",
            "slot:1",
            "--historical-provider",
            "alchemy",
            "--key",
            "env:M40_MISSING_KEY",
        ])
        .env_remove("M40_MISSING_KEY")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["error"]["code"], "SOURCE_CONFIGURATION", "{result}");
}

#[test]
fn retained_session_rejects_every_ignored_preparation_option() {
    let cases: &[&[&str]] = &[
        &["--historical-provider", "alchemy"],
        &["--historical-config", "missing.toml"],
        &["--key", "env:M40_MISSING_KEY"],
        &["--key-file", "missing.key"],
        &["--alchemy-config", "missing.json"],
        &["--collect", "all"],
        &["--max-requests", "1"],
        &["--max-download-bytes", "1"],
        &["--max-output-bytes", "1"],
        &[
            "--bank-input",
            "initializationEvidence",
            "--source",
            "missing.json",
            "--source-sha256",
            "00",
        ],
        &["--source", "missing.json", "--source-sha256", "00"],
        &[
            "--runtime-registry",
            "missing.json",
            "--runtime-registry-sha256",
            "00",
        ],
        &["--catalog", "missing.json", "--catalog-sha256", "00"],
        &["--bundle", "missing.json", "--bundle-sha256", "00"],
    ];
    for flags in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_svm-replay"))
            .args(["session", "--request", "missing-session.json"])
            .args(*flags)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{flags:?}: {:?}", output);
        assert!(
            output.stdout.is_empty(),
            "argument rejection must precede input I/O"
        );
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot be used"));
    }
}
