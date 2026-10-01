//! Offline admission tests; no provider or historical execution is substituted.
use serde_json::Value;
use std::process::Command;

fn invoke(args: &[&str]) -> std::process::Output {
    let directory = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .current_dir(directory.path())
        .args(args)
        .env_remove("API_ALCHEMY")
        .output()
        .unwrap()
}

#[test]
fn collect_all_and_json_validate_before_credentials_and_legacy_flags_reject() {
    for config in ["all", "{}", r#"{"calls":true}"#, r#"{"memory":true}"#] {
        failure(
            &["--tx", &"1".repeat(64), "--collect", config],
            "SOURCE_CONFIGURATION",
        );
    }
    failure(
        &["--tx", &"1".repeat(64), "--collect", r#"{"unknown":true}"#],
        "COLLECT_OPTIONS",
    );
    for flag in ["--trace", "--trace-options"] {
        assert_eq!(
            invoke(&["--tx", &"1".repeat(64), flag]).status.code(),
            Some(2)
        );
    }
}

#[test]
fn early_errors_are_saved_without_inventing_execution_receipts() {
    let root = tempfile::tempdir().unwrap();
    let out = root.path().join("results");
    let result = invoke(&["--tx", &"1".repeat(64), "--out", out.to_str().unwrap()]);
    assert!(!result.status.success());
    let summary: Value = serde_json::from_slice(&result.stdout).unwrap();
    let saved = summary["savedTo"].as_str().expect("saved error path");
    let error: Value = serde_json::from_slice(&std::fs::read(saved).unwrap()).unwrap();
    assert_eq!(error["error"]["code"], "SOURCE_CONFIGURATION");
    assert_eq!(error["schema"], "svm-replay-cli-error/v1");
    assert!(error.get("receiptPath").is_none());
    let invalid = invoke(&["--tx", "../../escape", "--out", out.to_str().unwrap()]);
    let summary: Value = serde_json::from_slice(&invalid.stdout).unwrap();
    assert_eq!(
        std::path::Path::new(summary["savedTo"].as_str().unwrap()).parent(),
        Some(out.as_path())
    );
}

#[test]
fn signature_accepts_explicit_bundle_and_requires_its_pin() {
    let help = String::from_utf8(invoke(&["--help"]).stdout).unwrap();
    assert!(help.contains("--bundle-sha256"));
    let output = invoke(&["--tx", &"1".repeat(64), "--bundle", "bundle.json"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("--bundle-sha256"));
}

#[test]
fn dotenv_loads_automatically_with_explicit_precedence() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join(".env");
    let run = |env: Option<&str>, key_file: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_svm-replay"));
        command
            .current_dir(directory.path())
            .args(["--tx", &"1".repeat(64)])
            .env_remove("API_ALCHEMY");
        if let Some(key) = env {
            command.env("API_ALCHEMY", key);
        }
        if let Some(path) = key_file {
            command.args(["--alchemy-key-file", path]);
        }
        let output = command.output().unwrap();
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains("test-secret"));
        serde_json::from_str::<Value>(&text).unwrap()["error"]["code"].clone()
    };
    for line in [
        "API_ALCHEMY=test-secret",
        "# comment\nexport API_ALCHEMY = 'test-secret' # comment\r\n",
        "API_ALCHEMY=\"test-secret\"\nOTHER=ignored",
    ] {
        std::fs::write(&file, line).unwrap();
        assert_eq!(run(None, None), "NOT_INSTALLED");
    }
    for line in [
        "API_ALCHEMY=",
        "API_ALCHEMY=\"unterminated",
        "API_ALCHEMY=$(echo test-secret)",
        "API_ALCHEMY=a\nAPI_ALCHEMY=b",
    ] {
        std::fs::write(&file, line).unwrap();
        assert_eq!(run(None, None), "SOURCE_CONFIGURATION");
        assert_eq!(run(Some("test-secret"), None), "NOT_INSTALLED");
    }
    std::fs::write(directory.path().join("key.txt"), "test-secret").unwrap();
    assert_eq!(run(Some(""), Some("key.txt")), "NOT_INSTALLED");
    std::fs::write(&file, "API_ALCHEMY=test-secret").unwrap();
    assert_eq!(run(Some(""), None), "SOURCE_CONFIGURATION");
    std::fs::write(&file, "OTHER=ignored").unwrap();
    assert_eq!(run(None, None), "SOURCE_CONFIGURATION");
}
fn failure(args: &[&str], code: &str) -> Value {
    let output = invoke(args);
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], code);
    value
}

#[test]
fn signature_admission_needs_no_network_or_manual_manifest() {
    let signature = "1".repeat(64);
    failure(&["--tx", &signature], "SOURCE_CONFIGURATION");
    failure(
        &["--tx", &signature, "--fields", "bogus"],
        "INVALID_REQUEST",
    );
    failure(
        &["--tx", &signature, "--fields", "trace"],
        "INVALID_REQUEST",
    );
    assert!(!invoke(&["--trace"]).status.success());
    let help = invoke(&["--help"]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    for flag in [
        "--tx",
        "--replace",
        "--overrides",
        "--out",
        "--demo",
        "--alchemy-key-file",
    ] {
        assert!(text.contains(flag), "{flag}");
    }
}

#[test]
fn experimental_debugger_is_not_in_the_signature_cli() {
    let output = invoke(&["--tx", &"1".repeat(64), "--debug"]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("unexpected argument '--debug'"));
    let help = invoke(&["--help"]);
    assert!(!String::from_utf8(help.stdout).unwrap().contains("--debug"));
}

#[test]
fn signature_controls_are_exposed_and_invalid_budgets_fail_before_network() {
    let help = String::from_utf8(invoke(&["--help"]).stdout).unwrap();
    for flag in [
        "--collect",
        "--timeout",
        "--max-requests",
        "--bank-input",
        "--source",
        "--runtime-registry",
    ] {
        assert!(help.contains(flag), "missing {flag}");
    }
    failure(
        &["--tx", &"1".repeat(64), "--timeout", "0"],
        "INVALID_REQUEST",
    );
    failure(
        &["--tx", &"1".repeat(64), "--max-requests", "0"],
        "INVALID_REQUEST",
    );
    failure(&["doctor"], "NOT_INSTALLED");
}

#[test]
fn public_api_alchemy_variable_reaches_installation() {
    let output = Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .args(["--tx", &"1".repeat(64)])
        .env("API_ALCHEMY", "test-key-never-sent")
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "NOT_INSTALLED");
    assert!(!value.to_string().contains("test-key-never-sent"));
}

#[test]
fn key_file_is_read_before_installation_and_never_echoed() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("test-key.txt");
    let signature = "1".repeat(64);
    // This source-built CLI has no installed bundle: a valid key reaches that
    // check, but never constructs a provider or sends this synthetic credential.
    std::fs::write(&path, "test-key-never-sent\n").unwrap();
    let args = [
        "--tx",
        &signature,
        "--alchemy-key-file",
        path.to_str().unwrap(),
    ];
    let value = failure(&args, "NOT_INSTALLED");
    assert!(!value.to_string().contains("test-key-never-sent"));
    for bytes in [b"".as_slice(), b"bad key", &[255u8]] {
        std::fs::write(&path, bytes).unwrap();
        failure(&args, "SOURCE_CONFIGURATION");
    }
}
