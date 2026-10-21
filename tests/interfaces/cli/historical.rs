//! Offline admission: synthetic keys never reach a transport or a worker.
use serde_json::Value;
use std::process::{Command, Output};

const SIGNATURE: &str = "1111111111111111111111111111111111111111111111111111111111111111";

#[test]
fn fork_can_emit_opt_in_transaction_events_without_changing_default_handshake() {
    let directory = tempfile::tempdir().unwrap();
    let output = invoke(directory.path(), &["fork", "--help"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("--events"));
    assert_eq!(
        invoke(directory.path(), &["fork", "--after", "slot:1", "--events"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn session_after_is_an_alternative_to_retained_inputs_and_needs_no_payload() {
    let directory = tempfile::tempdir().unwrap();
    for selector in ["01", "slot:01", "tx:bad"] {
        error(
            invoke(directory.path(), &["session", "--after", selector]),
            "INVALID_SELECTOR",
        );
    }
    error(
        invoke(directory.path(), &["session", "--after", "1"]),
        "SOURCE_CONFIGURATION",
    );
    assert_eq!(
        invoke(
            directory.path(),
            &["session", "--after", "tx:bad", "--request", "missing.json"]
        )
        .status
        .code(),
        Some(2)
    );
}

fn invoke(directory: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .current_dir(directory)
        .args(args)
        .env_remove("API_ALCHEMY")
        .env_remove("M40_HISTORICAL_TEST_KEY")
        .output()
        .unwrap()
}

fn error(output: Output, code: &str) -> Value {
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&output.stderr)));
    assert_eq!(value["error"]["code"], code);
    value
}

#[test]
fn after_requires_payload_and_conflicts_with_legacy_target() {
    let directory = tempfile::tempdir().unwrap();
    for args in [
        vec!["--after", "slot:1"],
        vec!["--payload", "{}"],
        vec!["--after", "slot:1", "--payload", "{}", "--tx", SIGNATURE],
        vec!["--after", "slot:1", "--payload", "{}", "--replace", "{}"],
        vec!["--after", "slot:1", "--payload", "{}", "--append", "{}"],
        vec!["--after", "slot:1", "--payload", "{}", "--demo"],
    ] {
        assert_eq!(
            invoke(directory.path(), &args).status.code(),
            Some(2),
            "{args:?}"
        );
    }
}

#[test]
fn historical_selectors_are_validated_before_payload_credentials_or_installation() {
    let directory = tempfile::tempdir().unwrap();
    for selector in [
        "01",
        "block:1",
        "slot:-1",
        "slot:01",
        "slot:9007199254740992",
        "tx:bad",
    ] {
        error(
            invoke(directory.path(), &["--after", selector, "--payload", "{}"]),
            "INVALID_SELECTOR",
        );
    }
    let selector = format!("tx:{SIGNATURE}");
    let value = error(
        invoke(directory.path(), &["--after", &selector, "--payload", "{}"]),
        "INVALID_REQUEST",
    );
    assert!(value["error"]["message"]
        .as_str()
        .unwrap()
        .contains("transactionBase64"));
}

#[test]
fn historical_provider_is_an_adapter_not_a_key_and_never_silently_falls_back() {
    let directory = tempfile::tempdir().unwrap();
    for provider in ["txlab", "invalid_provider", ""] {
        let value = error(
            invoke(
                directory.path(),
                &["--tx", SIGNATURE, "--historical-provider", provider],
            ),
            "SOURCE_CONFIGURATION",
        );
        assert!(!value.to_string().contains("synthetic-key-never-sent"));
    }
    let value = error(
        invoke(
            directory.path(),
            &[
                "--tx",
                SIGNATURE,
                "--historical-provider",
                "alchemy",
                "--historical-key",
                "synthetic-key-never-sent",
            ],
        ),
        "NOT_INSTALLED",
    );
    assert!(!value.to_string().contains("synthetic-key-never-sent"));
}

#[test]
fn provider_project_config_resolves_files_relative_to_config_and_redacts_parse_errors() {
    let directory = tempfile::tempdir().unwrap();
    let config_dir = directory.path().join("config");
    std::fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("svm-replay.toml");
    std::fs::write(config_dir.join("key.txt"), "synthetic-key-never-sent\n").unwrap();
    std::fs::write(&config, "[historical]\nprovider = 'archive'\n[historical.providers.archive]\nadapter = 'alchemy'\nkey_file = 'key.txt'\n").unwrap();
    let args = [
        "--tx",
        SIGNATURE,
        "--historical-config",
        config.to_str().unwrap(),
    ];
    let value = error(invoke(directory.path(), &args), "NOT_INSTALLED");
    assert!(!value.to_string().contains("synthetic-key-never-sent"));
    for text in [
        "[historical.providers.alchemy]\napi_key = 'synthetic-key-never-sent'\n",
        "[historical.providers.alchemy]\nkey_env = 'M40_HISTORICAL_TEST_KEY'\n",
        "[historical]\nprovider = 'unconfigured'\n",
        "[historical]\nprovider = 'synthetic-key-never-sent\n",
    ] {
        std::fs::write(&config, text).unwrap();
        let value = error(invoke(directory.path(), &args), "SOURCE_CONFIGURATION");
        assert!(!value.to_string().contains("synthetic-key-never-sent"));
    }
}

#[test]
fn explicit_provider_credentials_require_a_target_and_conflicting_keys_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    for args in [
        vec!["--historical-provider", "alchemy"],
        vec![
            "--tx",
            SIGNATURE,
            "--historical-key",
            "x",
            "--historical-key-file",
            "key.txt",
        ],
        vec![
            "--tx",
            SIGNATURE,
            "--historical-key",
            "x",
            "--alchemy-key-file",
            "key.txt",
        ],
    ] {
        assert_eq!(
            invoke(directory.path(), &args).status.code(),
            Some(2),
            "{args:?}"
        );
    }
}

#[test]
fn after_error_output_uses_the_anchor_signature_and_common_options() {
    let directory = tempfile::tempdir().unwrap();
    let selector = format!("tx:{SIGNATURE}");
    let output = invoke(
        directory.path(),
        &[
            "--after",
            &selector,
            "--payload",
            "{}",
            "--out",
            "saved",
            "--timeout",
            "10",
            "--fields",
            "status",
            "--no-cache",
        ],
    );
    let summary = error(output, "INVALID_REQUEST");
    let path = std::path::Path::new(summary["savedTo"].as_str().unwrap());
    assert!(path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .ends_with(&format!("-{SIGNATURE}.json")));
    assert!(directory.path().join(path).is_file());
}

#[test]
fn default_project_config_is_used_without_loading_dotenv_or_other_credential_sources() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("svm-replay.toml"),
        "[historical.providers.alchemy]\nkey_file='key.txt'\n",
    )
    .unwrap();
    std::fs::write(directory.path().join("key.txt"), "synthetic-key-never-sent").unwrap();
    std::fs::write(
        directory.path().join(".env"),
        "API_ALCHEMY=$(invalid-expression)",
    )
    .unwrap();
    let value = error(
        invoke(directory.path(), &["--tx", SIGNATURE]),
        "NOT_INSTALLED",
    );
    assert!(!value.to_string().contains("synthetic-key-never-sent"));
}
