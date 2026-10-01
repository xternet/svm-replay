use super::*;

#[test]
fn live_diagnostics_are_bounded_and_backpressure_reaps_worker() {
    let root = tempfile::tempdir().expect("test root");
    let spec = worker(
        root.path(),
        "printf 'listener-ready\\n' >&2; printf '{}' > \"$2\"",
    );
    let (sender, receiver) = std::sync::mpsc::sync_channel(2);
    let output = WorkerTransport::new(root.path())
        .run_with_diagnostics(
            &spec,
            &json!({}),
            &[],
            &[],
            &limits(),
            &budget(Duration::from_secs(2)),
            &sender,
        )
        .expect("streamed execution");
    assert_eq!(receiver.try_recv().expect("notice"), b"listener-ready\n");
    assert_eq!(output.stderr, b"listener-ready\n");
    let (blocked, _receiver) = std::sync::mpsc::sync_channel(0);
    let error = WorkerTransport::new(root.path())
        .run_with_diagnostics(
            &spec,
            &json!({}),
            &[],
            &[],
            &limits(),
            &budget(Duration::from_secs(2)),
            &blocked,
        )
        .expect_err("unconsumed notices fail closed");
    assert_eq!(error.code, WorkerErrorCode::DiagnosticLimit);
    assert_eq!(
        unsafe { libc::kill(error.pid.expect("started worker") as i32, 0) },
        -1
    );
}

#[test]
fn real_process_returns_json_with_no_inherited_environment() {
    let parent_home = std::env::var_os("HOME");
    let parent_group = unsafe { libc::getpgrp() };
    let mut subreaper_before: libc::c_int = 0;
    assert_eq!(
        unsafe { libc::prctl(libc::PR_GET_CHILD_SUBREAPER, &mut subreaper_before) },
        0
    );
    let root = tempfile::tempdir().expect("test root");
    let spec = worker(root.path(), "printf '{\"env\":\"%s\",\"key\":\"%s\"}' \"${HOME-unset}\" \"${API_ALCHEMY-unset}\" > \"$2\"; printf diagnostic >&2");
    let output = WorkerTransport::new(root.path())
        .run(
            &spec,
            &json!({"fixture": true}),
            &[],
            &limits(),
            &budget(Duration::from_secs(3)),
        )
        .expect("worker success");
    assert_eq!(output.value, json!({"env": "unset", "key": "unset"}));
    assert_eq!(output.stderr, b"diagnostic");
    assert_eq!(std::env::var_os("HOME"), parent_home);
    assert_eq!(unsafe { libc::getpgrp() }, parent_group);
    let mut subreaper_after: libc::c_int = 0;
    assert_eq!(
        unsafe { libc::prctl(libc::PR_GET_CHILD_SUBREAPER, &mut subreaper_after) },
        0
    );
    assert_eq!(
        subreaper_after, subreaper_before,
        "embedded library preserves host subreaper state"
    );
    assert_eq!(
        fs::read_dir(root.path()).expect("root listing").count(),
        1,
        "scratch removed after success"
    );
    assert_eq!(
        unsafe { libc::kill(output.pid as i32, 0) },
        -1,
        "immediate worker was reaped"
    );
}

#[test]
fn refuses_bad_pin_before_spawn_and_rechecks_pin_after_execution() {
    let root = tempfile::tempdir().expect("test root");
    let mut spec = worker(root.path(), "printf '{}' > \"$2\"");
    spec.sha256 = "0".repeat(64);
    let transport = WorkerTransport::new(root.path());
    let error = transport
        .run(
            &spec,
            &json!({}),
            &[],
            &limits(),
            &budget(Duration::from_secs(3)),
        )
        .expect_err("bad pin");
    assert_eq!(error.code, WorkerErrorCode::Integrity);
    assert!(error.pid.is_none());
    let spec = worker(
        root.path(),
        "printf '{}' > \"$2\"; printf '\\n# changed' >> \"$0\"",
    );
    let error = transport
        .run(
            &spec,
            &json!({}),
            &[],
            &limits(),
            &budget(Duration::from_secs(3)),
        )
        .expect_err("changed executable");
    assert_eq!(error.code, WorkerErrorCode::Integrity);
    assert!(error.pid.is_some());
}

#[test]
fn limits_combined_diagnostics_output_files_and_input() {
    for (body, field, expected) in [
        (
            "printf 123456; printf 789012 >&2; printf '{}' > \"$2\"",
            "diagnostic",
            WorkerErrorCode::DiagnosticLimit,
        ),
        (
            "printf 123456789012 > \"$2\"",
            "output",
            WorkerErrorCode::OutputLimit,
        ),
        ("printf '{}' > \"$2\"", "input", WorkerErrorCode::InputLimit),
    ] {
        let root = tempfile::tempdir().expect("test root");
        let spec = worker(root.path(), body);
        let mut bounds = limits();
        match field {
            "diagnostic" => bounds.max_diagnostic_bytes = 10,
            "output" => bounds.max_output_bytes = 10,
            "input" => bounds.max_input_bytes = 1,
            _ => unreachable!(),
        }
        let error = WorkerTransport::new(root.path())
            .run(
                &spec,
                &json!({}),
                &[],
                &bounds,
                &budget(Duration::from_secs(3)),
            )
            .expect_err("limit");
        assert_eq!(error.code, expected, "{error}");
        assert!(error.stdout.len() + error.stderr.len() <= bounds.max_diagnostic_bytes);
        assert_eq!(
            fs::read_dir(root.path()).expect("root listing").count(),
            1,
            "scratch removed on failure"
        );
    }
}
