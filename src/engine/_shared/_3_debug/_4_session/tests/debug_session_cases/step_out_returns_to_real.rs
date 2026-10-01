use super::*;

#[test]
fn step_out_returns_to_real_suspended_parent_and_continue_all_finishes_worker() {
    let root = tempfile::tempdir().expect("root");
    let (worker, port) = setup(root.path());
    let transport = WorkerTransport::new(root.path());
    let budget =
        ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).expect("budget");
    let (controller, mut driver) = channel();
    thread::scope(|scope| {
        let running = scope.spawn(|| {
            run_session(
                &transport,
                &worker,
                &json!({}),
                &[port.to_string()],
                &WorkerLimits::default(),
                &budget,
                port,
                64,
                "original-control",
                &mut driver,
            )
        });
        let mut request_id = 0;
        let mut parent_stopped = false;
        loop {
            let event = controller.next_event(&budget).expect("live event");
            let action = if event["kind"] == "entry" {
                Some(if event["target"]["invocationIndex"] == 0 {
                    DebugAction::Continue
                } else {
                    DebugAction::StepOut
                })
            } else if event["kind"] == "stop" && event["target"]["invocationIndex"] == 0 {
                assert_eq!(event["stop"]["kind"], "stopped");
                assert_eq!(event["stop"]["signal"], 2);
                parent_stopped = true;
                Some(DebugAction::ContinueAll)
            } else {
                None
            };
            if let Some(action) = action {
                request_id += 1;
                let all = matches!(action, DebugAction::ContinueAll);
                controller
                    .send(DebugCommand {
                        request_id,
                        phase: "original-control".into(),
                        target: if all { None } else { Some(target(&event)) },
                        action,
                    })
                    .expect("navigation");
                if all {
                    break;
                }
            }
        }
        let result = running
            .join()
            .expect("session thread")
            .expect("worker completed");
        assert!(parent_stopped);
        assert_eq!(result.invocations.len(), 2);
        assert_eq!(
            result.worker.value["computed"],
            "all live fixture VMs exited"
        );
        assert!(
            result
                .events
                .iter()
                .all(|event| event["kind"] != "transaction-complete"),
            "VM exit cannot certify a transaction"
        );
    });
}

#[test]
fn cancellation_while_really_paused_closes_transport_and_reaps_worker() {
    let root = tempfile::tempdir().expect("root");
    let (worker, port) = setup(root.path());
    let transport = WorkerTransport::new(root.path());
    let cancellation = CancellationToken::new();
    let budget =
        ExecutionBudget::new(Duration::from_secs(5), cancellation.clone()).expect("budget");
    let (controller, mut driver) = channel();
    thread::scope(|scope| {
        let running = scope.spawn(|| {
            run_session(
                &transport,
                &worker,
                &json!({}),
                &[port.to_string()],
                &WorkerLimits::default(),
                &budget,
                port,
                64,
                "original-control",
                &mut driver,
            )
        });
        loop {
            let event = controller.next_event(&budget).expect("entry event");
            if event["kind"] == "entry" {
                assert_eq!(event["state"], "STOPPED");
                cancellation.cancel();
                break;
            }
        }
        let error = running
            .join()
            .expect("session thread")
            .err()
            .expect("cancellation");
        assert_eq!(error.code, "WORKER_CANCELLED");
        let details = error.details.expect("teardown diagnostics");
        let pid = details["workerError"]["details"]["pid"]
            .as_i64()
            .expect("worker pid");
        assert_eq!(
            unsafe { libc::kill(pid as i32, 0) },
            -1,
            "paused immediate worker reaped"
        );
        assert!(details["events"]
            .as_array()
            .expect("partial events")
            .iter()
            .all(|event| event["kind"] != "transaction-complete"));
    });
}

#[test]
fn non_loopback_listener_notice_is_rejected_without_connecting() {
    let root = tempfile::tempdir().expect("root");
    let (mut worker, port) = setup(root.path());
    fs::write(
        &worker.executable,
        WORKER.replace(
            "Waiting for bounded debugger on 127.0.0.1:",
            "Waiting for bounded debugger on 0.0.0.0:",
        ),
    )
    .expect("invalid-notice fixture");
    worker.sha256 = file_sha256(&worker.executable).expect("explicit fixture pin");
    let transport = WorkerTransport::new(root.path());
    let budget =
        ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).expect("budget");
    let (_controller, mut driver) = channel();
    let error = run_session(
        &transport,
        &worker,
        &json!({}),
        &[port.to_string()],
        &WorkerLimits::default(),
        &budget,
        port,
        64,
        "original-control",
        &mut driver,
    )
    .err()
    .expect("invalid endpoint rejected");
    assert_eq!(error.code, "DEBUG_ENDPOINT");
    let pid = error.details.expect("teardown proof")["workerError"]["details"]["pid"]
        .as_i64()
        .expect("pid");
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
}
