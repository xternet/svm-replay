use super::*;

pub(super) fn call(root: &Path, mut request: Value) -> Value {
    request["version"] = json!(1);
    let mut child = Command::new(env!("CARGO_BIN_EXE_svm-replay-store"))
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn real cache process");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(request.to_string().as_bytes())
        .expect("write request");
    let output = child.wait_with_output().expect("wait cache");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "response: {e}; stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_eq!(
        output.status.success(),
        value["status"] != "ERROR",
        "exit and outcome disagree: {value}"
    );
    value
}

pub(super) fn put(root: &Path, ns: &str, key: &str, bytes: &[u8]) -> Value {
    call(
        root,
        json!({"op":"put","namespace":ns,"key":key,"dataBase64":STANDARD.encode(bytes)}),
    )
}

pub(super) fn get(root: &Path, ns: &str, key: &str) -> Value {
    call(root, json!({"op":"get","namespace":ns,"key":key}))
}
