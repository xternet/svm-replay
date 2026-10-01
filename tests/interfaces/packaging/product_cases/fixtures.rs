use super::*;

pub(super) fn cli(args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .args(args)
        .output()
        .unwrap();
    let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "{e}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.success(), value)
}

#[cfg(target_os = "linux")]
pub(super) fn catalog(root: &Path) -> String {
    // Protocol/installer test, not a historical runtime qualification.
    fs::copy("/usr/bin/true", root.join("worker")).unwrap();
    let hash = Digest::of(fs::read(root.join("worker")).unwrap());
    let value = json!({"schema":"svm-replay-workers/v1","platform":{"os":"linux","arch":std::env::consts::ARCH,"glibcMin":"2.38"},
        "workers":[{"family":"v4-2","file":"worker","sha256":hash,"buildHash":hash,"sourceSha256":hash,
        "executorSourceId":"installer-test-only","capabilities":["protocol-test-only"]}]});
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(root.join("catalog.json"), &bytes).unwrap();
    Digest::of(bytes).as_str().to_owned()
}

pub(super) fn response_pin(value: &Value) -> Digest {
    Digest::of(serde_json::to_vec(value).unwrap())
}
