//! JSON presentation must not change values, exit codes or evidence files.
use serde_json::Value;
use std::process::{Command, Output};

fn invoke(args: &[&str]) -> Output {
    let cwd = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .current_dir(cwd.path())
        .env_remove("API_ALCHEMY")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn pretty_stdout_preserves_json_and_failure_exit() {
    let tx = "1".repeat(64);
    let compact = invoke(&["--tx", &tx]);
    let pretty = invoke(&["--tx", &tx, "--pretty"]);
    assert_eq!(compact.status.code(), pretty.status.code());
    let expected: Value = serde_json::from_slice(&compact.stdout).unwrap();
    let actual: Value = serde_json::from_slice(&pretty.stdout).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual["error"]["code"], "SOURCE_CONFIGURATION");
    assert_eq!(
        String::from_utf8(pretty.stdout).unwrap(),
        format!("{}\n", serde_json::to_string_pretty(&actual).unwrap())
    );
    assert_eq!(
        String::from_utf8(compact.stdout).unwrap().lines().count(),
        1
    );
}

#[test]
fn pretty_saved_output_and_acknowledgement_are_indented() {
    let directory = tempfile::tempdir().unwrap();
    let output = invoke(&[
        "--tx",
        &"1".repeat(64),
        "--pretty",
        "--json",
        "--out",
        directory.path().to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let ack: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{}\n", serde_json::to_string_pretty(&ack).unwrap())
    );
    let text = std::fs::read_to_string(ack["savedTo"].as_str().unwrap()).unwrap();
    let saved: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(text, serde_json::to_string_pretty(&saved).unwrap());
    assert_eq!(saved["error"]["code"], "SOURCE_CONFIGURATION");
}

#[test]
fn pretty_conflicts_with_human_and_never_changes_debug_jsonl() {
    assert_eq!(invoke(&["--pretty", "--human"]).status.code(), Some(2));
    let output = invoke(&[
        "simulate",
        "--request",
        "missing.json",
        "--debug",
        "--trace",
        "missing-trace.json",
        "--pretty",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["error"]["code"], "INVALID_REQUEST");
    assert!(error["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--pretty"));
}

#[cfg(unix)]
fn terminal(args: &[&str]) -> String {
    use std::io::Read;
    use std::os::fd::FromRawFd;
    let (mut master, mut slave) = (-1, -1);
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let mut reader = unsafe { std::fs::File::from_raw_fd(master) };
    let writer = unsafe { std::fs::File::from_raw_fd(slave) };
    let cwd = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_svm-replay"));
    command
        .current_dir(cwd.path())
        .env_remove("API_ALCHEMY")
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(writer)
        .stderr(std::process::Stdio::null());
    let mut child = command.spawn().unwrap();
    drop(command);
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
            Err(error) => panic!("terminal read: {error}"),
        }
    }
    assert_eq!(child.wait().unwrap().code(), Some(1));
    String::from_utf8(bytes).unwrap().replace("\r\n", "\n")
}

#[test]
#[cfg(unix)]
fn terminal_defaults_pretty_but_saved_json_stays_compact() {
    let directory = tempfile::tempdir().unwrap();
    let text = terminal(&[
        "--tx",
        &"1".repeat(64),
        "--out",
        directory.path().to_str().unwrap(),
    ]);
    let ack: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        text,
        format!("{}\n", serde_json::to_string_pretty(&ack).unwrap())
    );
    let saved = std::fs::read_to_string(ack["savedTo"].as_str().unwrap()).unwrap();
    let value: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved, serde_json::to_string(&value).unwrap());
}

#[test]
#[cfg(unix)]
fn json_forces_compact_terminal_unless_pretty_is_explicit() {
    let tx = "1".repeat(64);
    let compact = terminal(&["--tx", &tx, "--json"]);
    assert_eq!(compact.lines().count(), 1);
    let pretty = terminal(&["--tx", &tx, "--json", "--pretty"]);
    let value: Value = serde_json::from_str(&pretty).unwrap();
    assert_eq!(
        pretty,
        format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
    );
}
