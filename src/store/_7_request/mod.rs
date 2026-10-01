use super::*;

pub fn parse_request(bytes: &[u8]) -> Result<Value> {
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err(StoreError::new("INPUT_TOO_LARGE", "stdin exceeds 360 MiB"));
    }
    serde_json::from_slice(bytes).map_err(|e| StoreError::new("INVALID_REQUEST", e.to_string()))
}

#[cfg(test)]
pub(super) fn crash_point(point: &str) {
    if std::env::var("SVM_REPLAY_STORE_TEST_CRASH").as_deref() == Ok(point) {
        #[cfg(unix)]
        std::process::Command::new("/bin/kill")
            .args(["-KILL", &std::process::id().to_string()])
            .status()
            .expect("send real SIGKILL to test subprocess");
        #[cfg(windows)]
        std::process::Command::new("taskkill")
            .args(["/F", "/PID", &std::process::id().to_string()])
            .status()
            .expect("force-terminate the test subprocess");
        panic!("forced termination did not terminate the child");
    }
}
