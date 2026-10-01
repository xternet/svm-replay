use super::*;

#[test]
fn real_sigkill_during_transport_does_not_refund_committed_reservation() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cache_fill_uses_distinct_handle::reservation_crash_child",
            "--ignored",
            "--nocapture",
        ])
        .env("SVM_REPLAY_RPC_BUDGET_CRASH_ROOT", dir.path())
        .output()
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            output.status.signal(),
            Some(9),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[cfg(windows)]
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let limits = SourceLimits {
        max_download_bytes: 1024,
        ..limits()
    };
    let ledger = budget(dir.path(), &limits);
    assert_eq!(
        ledger.snapshot().unwrap()["reserved"],
        json!({"reads":0,"requests":1,"bytes":1024})
    );
    let no_network = Transport::new(dir.path(), vec![]);
    let source = source(limits, no_network.clone(), ledger);
    assert_eq!(
        source.inspect(&query(2)).unwrap_err().code,
        "SOURCE_RESOURCE_LIMIT"
    );
    assert_eq!(no_network.calls.load(Ordering::SeqCst), 0);
}
