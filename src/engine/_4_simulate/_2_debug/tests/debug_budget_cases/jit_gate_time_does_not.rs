use super::*;

#[test]
fn jit_gate_time_does_not_consume_a_fresh_live_session_cap() {
    let root = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let result = run(root.path(), 5000, 1000, CancellationToken::new())
        .expect("gate and live worker each fit their own capture cap");
    assert!(started.elapsed() > Duration::from_millis(1000));
    assert_eq!(result.receipt["status"], "PASS");
    let gate = if cfg!(target_arch = "x86_64") {
        "jitGate"
    } else {
        "nativeGate"
    };
    assert_eq!(result.receipt[gate]["workerCalls"], 3);
    assert_eq!(result.receipt["debugWorkerCalls"], 1);
    assert_eq!(result.receipt["deadlinePolicy"]["captureTimeoutMs"], 1000);
    assert_eq!(
        result.control_evidence["provenance"]["liveInterpreterParityEstablished"],
        true
    );
    assert_eq!(
        fs::read_to_string(root.path().join("calls"))
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        vec!["--discover-sysvars", "off", "--capture", "live"]
    );
}

#[test]
fn job_deadline_and_cancellation_still_dominate_fresh_live_budget() {
    let root = tempfile::tempdir().unwrap();
    let error = run(root.path(), 1000, 5000, CancellationToken::new())
        .err()
        .expect("whole job deadline still applies");
    assert_eq!(error.code, "WORKER_TIMEOUT");
    let proof = &error.details.as_ref().unwrap()["controlEvidence"];
    assert_eq!(
        proof["complete"], true,
        "independent passed JIT control must survive a live deadline"
    );
    assert_eq!(
        proof["provenance"]["executionMode"],
        if cfg!(target_arch = "x86_64") {
            "jit"
        } else {
            "interpreter"
        }
    );
    assert_eq!(
        proof["provenance"]["liveInterpreterParityEstablished"],
        false
    );
    let pid = fs::read_to_string(root.path().join("live-started"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "deadline reaped live worker"
    );
    let root = tempfile::tempdir().unwrap();
    let token = CancellationToken::new();
    thread::scope(|scope| {
        let control = token.clone();
        let path = root.path().join("live-started");
        let canceller = scope.spawn(move || {
            let started = Instant::now();
            while !path.exists() {
                assert!(
                    started.elapsed() < Duration::from_secs(5),
                    "live worker startup missing"
                );
                thread::sleep(Duration::from_millis(5));
            }
            control.cancel();
        });
        let error = run(root.path(), 5000, 1000, token)
            .err()
            .expect("cancellation shares job token");
        canceller.join().unwrap();
        assert_eq!(error.code, "WORKER_CANCELLED");
        let proof = &error.details.as_ref().unwrap()["controlEvidence"];
        assert_eq!(proof["complete"], true);
        assert_eq!(
            proof["provenance"]["executionMode"],
            if cfg!(target_arch = "x86_64") {
                "jit"
            } else {
                "interpreter"
            }
        );
        assert_eq!(
            proof["provenance"]["liveInterpreterParityEstablished"],
            false
        );
    });
    let pid = fs::read_to_string(root.path().join("live-started"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "cancelled live worker reaped"
    );
}

#[test]
fn a_live_session_still_cannot_exceed_its_own_capture_cap() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("stall-live"),
        b"explicit protocol fixture stall",
    )
    .unwrap();
    let error = run(root.path(), 5000, 1000, CancellationToken::new())
        .err()
        .expect("live capture sub-budget enforced");
    assert_eq!(error.code, "WORKER_TIMEOUT");
    let pid = fs::read_to_string(root.path().join("live-started"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "capture cap reaped live worker"
    );
    assert_eq!(
        error.details.unwrap()["controlEvidence"]["provenance"]["liveInterpreterParityEstablished"],
        false
    );
}
