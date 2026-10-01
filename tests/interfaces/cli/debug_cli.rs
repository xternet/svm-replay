#![cfg(target_os = "linux")]
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use svm_replay_protocol::Digest;

// Real CLI failure protocol: a held-open stdin must not create a reader thread
// that prevents option/admission failures from returning. Not an SVM oracle.
#[test]
fn debug_requires_trace_and_invalid_options_exit_without_waiting_for_stdin() {
    let root = tempfile::tempdir().unwrap();
    let request = root.path().join("request.json");
    fs::write(
        &request,
        serde_json::to_vec(&json!({"schema":"svm-replay-prepared/v1"})).unwrap(),
    )
    .unwrap();
    for has_trace in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_svm-replay"));
        command
            .args(["simulate", "--debug", "--request"])
            .arg(&request)
            .arg("--catalog")
            .arg(root.path().join("missing-catalog.json"))
            .args(["--catalog-sha256", Digest::of(b"missing").as_str()])
            .arg("--data-dir")
            .arg(root.path().join("data"));
        if has_trace {
            let trace = root.path().join("trace.json");
            fs::write(&trace, b"{}").unwrap();
            command.arg("--trace").arg(trace);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap(); // Kept open throughout the wait.
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("CLI input reader prevented error exit");
            }
            thread::sleep(Duration::from_millis(5));
        };
        drop(input);
        assert!(!status.success());
        let mut stdout = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut stdout)
            .unwrap();
        if has_trace {
            let lines: Vec<_> = stdout.lines().collect();
            assert_eq!(lines.len(), 1);
            let receipt: Value = serde_json::from_str(lines[0]).unwrap();
            assert_eq!(receipt["outcome"], "ERROR");
            assert_eq!(receipt["error"]["code"], "TRACE_OPTIONS");
        } else {
            assert!(stdout.is_empty());
            let mut stderr = String::new();
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut stderr)
                .unwrap();
            assert!(stderr.contains("--trace"));
        }
    }
}
