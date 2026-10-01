use super::*;

#[test]
fn timeout_and_cancellation_kill_and_wait_immediate_worker() {
    for cancel in [false, true] {
        let root = tempfile::tempdir().expect("test root");
        let spec = worker(root.path(), "while :; do :; done");
        let cancellation = CancellationToken::new();
        let run_budget = ExecutionBudget::new(
            if cancel {
                Duration::from_secs(3)
            } else {
                Duration::from_millis(100)
            },
            cancellation.clone(),
        )
        .expect("budget");
        let canceller = cancel.then(|| {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                cancellation.cancel();
            })
        });
        let started = Instant::now();
        let error = WorkerTransport::new(root.path())
            .run(&spec, &json!({}), &[], &limits(), &run_budget)
            .expect_err("stopped worker");
        if let Some(handle) = canceller {
            handle.join().expect("canceller thread");
        }
        assert_eq!(
            error.code,
            if cancel {
                WorkerErrorCode::Cancelled
            } else {
                WorkerErrorCode::Timeout
            },
            "{error}"
        );
        let pid = error.pid.expect("spawned worker");
        assert_eq!(
            unsafe { libc::kill(pid as i32, 0) },
            -1,
            "worker no longer exists"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}

#[test]
fn flooded_diagnostics_cannot_block_cancellation_or_escape_byte_limit() {
    let root = tempfile::tempdir().expect("test root");
    let spec = worker(
        root.path(),
        "while :; do printf 0123456789012345678901234567890123456789; done",
    );
    let error = WorkerTransport::new(root.path())
        .run(
            &spec,
            &json!({}),
            &[],
            &limits(),
            &budget(Duration::from_secs(3)),
        )
        .expect_err("diagnostic flood");
    assert_eq!(error.code, WorkerErrorCode::DiagnosticLimit, "{error}");
    assert!(error.stdout.len() + error.stderr.len() <= limits().max_diagnostic_bytes);
    assert_eq!(
        unsafe { libc::kill(error.pid.expect("worker pid") as i32, 0) },
        -1
    );
}

#[test]
fn budget_is_cumulative_and_precancellation_never_spawns() {
    let root = tempfile::tempdir().expect("test root");
    let spec = worker(root.path(), "printf '{}' > \"$2\"");
    let transport = WorkerTransport::new(root.path());
    let run_budget = budget(Duration::from_millis(50));
    thread::sleep(Duration::from_millis(60));
    let error = transport
        .run(&spec, &json!({}), &[], &limits(), &run_budget)
        .expect_err("expired shared budget");
    assert_eq!(error.code, WorkerErrorCode::Timeout);
    assert!(error.pid.is_none());
    let token = CancellationToken::new();
    token.cancel();
    let run_budget = ExecutionBudget::new(Duration::from_secs(1), token).expect("budget");
    let error = transport
        .run(&spec, &json!({}), &[], &limits(), &run_budget)
        .expect_err("precancelled");
    assert_eq!(error.code, WorkerErrorCode::Cancelled);
    assert!(error.pid.is_none());
}

#[test]
fn relative_scratch_root_is_resolved_before_child_changes_directory() {
    let root = tempfile::tempdir().expect("test root");
    let spec = worker(root.path(), "printf '{}' > \"$2\"");
    let output = WorkerTransport::new(".")
        .run(
            &spec,
            &json!({}),
            &[],
            &limits(),
            &budget(Duration::from_secs(3)),
        )
        .expect("tempfile resolves scratch path before spawn");
    assert_eq!(output.value, json!({}));
}

#[test]
fn auxiliary_checkpoint_files_are_bounded_during_execution() {
    let root = tempfile::tempdir().expect("test root");
    let checkpoint = root.path().join("checkpoint.json");
    let spec = worker(
        root.path(),
        "printf 12345678901234567890 > \"$3\"; while :; do :; done",
    );
    let mut bounds = limits();
    bounds.max_output_bytes = 10;
    let error = WorkerTransport::new(root.path())
        .run_with_files(
            &spec,
            &json!({}),
            &[checkpoint.display().to_string()],
            std::slice::from_ref(&checkpoint),
            &bounds,
            &budget(Duration::from_secs(3)),
        )
        .expect_err("checkpoint file limit");
    assert_eq!(error.code, WorkerErrorCode::OutputLimit, "{error}");
    assert!(fs::metadata(checkpoint).expect("partial checkpoint").len() <= 10);
    assert_eq!(
        unsafe { libc::kill(error.pid.expect("worker pid") as i32, 0) },
        -1
    );
}

#[test]
fn rejects_nonregular_results_and_propagates_exit_diagnostics() {
    for (body, expected) in [
        ("/usr/bin/ln -s \"$1\" \"$2\"", WorkerErrorCode::Io),
        ("printf not-json > \"$2\"", WorkerErrorCode::InvalidOutput),
        (
            "printf '{\"key\":1,\"key\":2}' > \"$2\"",
            WorkerErrorCode::InvalidOutput,
        ),
        ("printf failure >&2; exit 17", WorkerErrorCode::WorkerExit),
    ] {
        let root = tempfile::tempdir().expect("test root");
        let spec = worker(root.path(), body);
        let error = WorkerTransport::new(root.path())
            .run(
                &spec,
                &json!({}),
                &[],
                &limits(),
                &budget(Duration::from_secs(3)),
            )
            .expect_err("invalid result");
        assert_eq!(error.code, expected, "{error}");
        if expected == WorkerErrorCode::WorkerExit {
            assert_eq!(error.stderr, b"failure");
            assert!(error.message.contains("17"));
        }
    }
}

#[test]
fn nested_budget_can_only_shorten_deadline_and_keeps_cancellation() {
    let token = CancellationToken::new();
    let parent = ExecutionBudget::new(Duration::from_secs(5), token.clone()).unwrap();
    let child = parent.narrowed(Duration::from_millis(15)).unwrap();
    let extended = child.narrowed(Duration::from_secs(5)).unwrap();
    std::thread::sleep(Duration::from_millis(25));
    assert!(parent.check().is_ok());
    assert_eq!(extended.check().unwrap_err().code, WorkerErrorCode::Timeout);
    let shared = parent.narrowed(Duration::from_secs(1)).unwrap();
    token.cancel();
    assert_eq!(shared.check().unwrap_err().code, WorkerErrorCode::Cancelled);
}
