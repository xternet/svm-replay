use super::*;
#[test]
fn default_deadline_covers_large_acquisitions_and_remains_overridable() {
    assert_eq!(Controls::default().limits().unwrap().timeout_ms, 900_000);
    let shorter = Controls {
        timeout: Some(120),
        ..Default::default()
    };
    assert_eq!(shorter.limits().unwrap().timeout_ms, 120_000);
    for seconds in [1800, 3600] {
        let extended = Controls {
            timeout: Some(seconds),
            ..Default::default()
        };
        assert_eq!(extended.limits().unwrap().timeout_ms, seconds * 1000);
    }
    for seconds in [0, 3601, u64::MAX] {
        assert!(Controls {
            timeout: Some(seconds),
            ..Default::default()
        }
        .limits()
        .is_err());
    }
}

#[test]
fn custom_registry_rejects_wrong_pin_and_invalid_content() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("registry.json");
    std::fs::write(&path, b"{}").unwrap();
    let mut controls = Controls {
        runtime_registry: Some(path),
        runtime_registry_sha256: Some(Digest::of(b"wrong").as_str().into()),
        ..Default::default()
    };
    assert!(controls
        .registry()
        .err()
        .unwrap()
        .message
        .contains("hash differs"));
    controls.runtime_registry_sha256 = Some(Digest::of(b"{}").as_str().into());
    assert_eq!(controls.registry().err().unwrap().code, "RUNTIME_REGISTRY");
}
#[test]
fn elapsed_acquisition_is_not_a_fresh_execution_budget() {
    let limits = Controls {
        timeout: Some(1),
        ..Default::default()
    }
    .limits()
    .unwrap();
    let start = Instant::now() - Duration::from_secs(2);
    assert_eq!(
        remaining(start, &limits).unwrap_err().code,
        "WORKER_TIMEOUT"
    );
}
