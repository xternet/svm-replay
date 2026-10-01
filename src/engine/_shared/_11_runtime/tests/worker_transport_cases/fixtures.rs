use super::*;

pub(super) fn worker(root: &Path, body: &str) -> WorkerSpec {
    let path = root.join("test-worker");
    // This is an explicit test fixture, never a PATH-shadowed production command.
    fs::write(&path, format!("#!/usr/bin/dash\n{body}\n")).expect("write fixture");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("fixture permissions");
    WorkerSpec {
        sha256: svm_replay_engine::shared::runtime::file_sha256(&path).expect("fixture digest"),
        executable: path,
    }
}

pub(super) fn limits() -> WorkerLimits {
    WorkerLimits {
        max_input_bytes: 4096,
        max_output_bytes: 4096,
        max_diagnostic_bytes: 4096,
        cleanup_grace: Duration::from_secs(1),
    }
}

pub(super) fn budget(timeout: Duration) -> ExecutionBudget {
    ExecutionBudget::new(timeout, CancellationToken::new()).expect("budget")
}
