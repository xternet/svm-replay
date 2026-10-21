//! Help readability: grouped options, a description for each, `fork` listed, no colors when piped.
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

fn text(args: &[&str]) -> String {
    let output = invoke(args);
    assert!(output.status.success(), "{args:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn long_help_groups_options_and_describes_each_one() {
    let help = text(&["--help"]);
    for heading in [
        "Commands:",
        "Replay options:",
        "What-if options:",
        "Output options:",
        "Provider and cache options:",
        "Limit options:",
        "Pinned input options (advanced):",
        "Examples:",
    ] {
        assert!(help.contains(heading), "missing {heading}");
    }
    assert!(help.lines().any(|line| line.starts_with("  fork ")));
    assert!(!help.contains('\x1b'), "piped help has no colors");
    // Long layout: every option line is followed by its indented description.
    let lines: Vec<&str> = help.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let indent = line.len() - line.trim_start().len();
        if line.trim_start().starts_with('-') && indent <= 6 {
            let next = lines.get(index + 1).copied().unwrap_or_default();
            assert!(
                next.starts_with("          ") && !next.trim().is_empty(),
                "no description for {line}"
            );
        }
    }
}

/// Every option stands on its own line with its description below, and a blank line separates it
/// from the next one.
fn assert_spaced(help: &str, page: &str) {
    let lines: Vec<&str> = help.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let item = line.trim_start();
        let indent = line.len() - item.len();
        if indent > 6 || item.is_empty() || line.ends_with(':') || item.starts_with("svm-replay") {
            continue;
        }
        let next = lines.get(index + 1).copied().unwrap_or_default();
        if item.starts_with('-') {
            let previous = lines[index.saturating_sub(1)];
            assert!(
                previous.trim().is_empty() || previous.ends_with(':'),
                "{page}: no blank line before {line}"
            );
            assert!(
                next.starts_with("          ") && !next.trim().is_empty(),
                "{page}: no description under {line}"
            );
        }
    }
}

#[test]
fn every_help_page_is_spaced_and_short_help_is_the_same_page() {
    let long = text(&["--help"]);
    assert_eq!(text(&["-h"]), long, "-h prints the same readable page");
    assert_spaced(&long, "svm-replay --help");
    for command in ["doctor", "simulate", "bundle"] {
        let page = text(&[command, "--help"]);
        assert_eq!(text(&[command, "-h"]), page, "{command} -h");
        assert_spaced(&page, command);
    }
}

#[test]
fn native_fork_owns_help_and_selector_admission_without_the_npm_adapter() {
    assert!(text(&["fork", "--help"]).contains("--after"));
    let output = invoke(&["fork", "--after", "tx:bad"]);
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "INVALID_SELECTOR");
}

#[test]
fn documented_commands_have_help_and_required_inputs() {
    for (command, required) in [
        ("tx", "<SIGNATURE>"),
        ("payload", "<PAYLOAD>"),
        ("fork", "--after"),
        ("runtimes", "runtimes"),
        ("completions", "<SHELL>"),
    ] {
        let page = text(&[command, "--help"]);
        assert!(page.contains(required), "{command}: missing {required}");
        assert!(!page.contains('\x1b'));
    }
    let tx = text(&["tx", "--help"]);
    for option in [
        "--state-overrides",
        "--provider",
        "--key",
        "--config",
        "--data-dir",
        "--human",
        "--collect",
        "--fields",
        "--out",
        "--progress",
    ] {
        assert!(tx.contains(option), "tx: missing {option}");
    }
    let payload = text(&["payload", "--help"]);
    assert!(payload.contains("--state-overrides"));
    assert!(payload.contains("--after"));
    assert!(text(&["fork", "--help"]).contains("--port"));
    for command in [None, Some("payload"), Some("fork")] {
        let args = match command {
            Some(command) => vec![command, "--help"],
            None => vec!["--help"],
        };
        let page = text(&args);
        assert!(page.contains("--after <SIGNATURE|SLOT>"), "{page}");
        assert!(!page.contains("tx:"), "{page}");
        assert!(!page.contains("slot:"), "{page}");
    }
}

#[test]
fn documented_missing_inputs_fail_in_admission_not_as_unknown_commands() {
    for args in [
        vec!["tx"],
        vec!["payload"],
        vec!["payload", "{}"],
        vec!["payload", "--after", "slot:1"],
        vec!["fork"],
        vec!["fork", "--request", "request.json"],
        vec!["tx", "signature", "--after", "slot:1"],
    ] {
        let output = invoke(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            !stderr.contains("unrecognized subcommand"),
            "{args:?}: {stderr}"
        );
    }
}

#[test]
fn canonical_tx_uses_the_existing_signature_engine() {
    let legacy = invoke(&["--tx", "invalid-signature"]);
    let canonical = invoke(&["tx", "invalid-signature"]);
    let legacy: Value = serde_json::from_slice(&legacy.stdout).unwrap();
    let canonical: Value = serde_json::from_slice(&canonical.stdout).unwrap();
    assert_eq!(canonical["error"]["code"], legacy["error"]["code"]);
}

#[test]
fn bare_after_selectors_reach_payload_validation_and_keep_signature_in_saved_errors() {
    let signature = bs58::encode([7_u8; 64]).into_string();
    for selector in ["123", "0", "9007199254740991", signature.as_str()] {
        let output = invoke(&["payload", "{}", "--after", selector]);
        assert!(!output.status.success());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], "INVALID_REQUEST");
    }
    let directory = tempfile::tempdir().unwrap();
    let output = invoke(&[
        "payload",
        "{}",
        "--after",
        &signature,
        "--out",
        directory.path().to_str().unwrap(),
    ]);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "INVALID_REQUEST");
    assert!(value["savedTo"]
        .as_str()
        .unwrap()
        .ends_with(&format!("-{signature}.json")));
}

#[test]
fn documented_bash_completions_are_shell_syntax_not_a_receipt() {
    let output = invoke(&["completions", "bash"]);
    assert!(output.status.success());
    let script = String::from_utf8(output.stdout).unwrap();
    assert!(script.contains("svm-replay"));
    assert!(!script.contains("\"outcome\""));
    let bash = if cfg!(windows) {
        std::env::var_os("SVM_REPLAY_TEST_BASH")
            .expect("SVM_REPLAY_TEST_BASH must name Git Bash on Windows")
    } else {
        "bash".into()
    };
    let mut child = Command::new(bash)
        .arg("-n")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
}
