use super::*;

#[test]
fn source_command_without_admitted_symbols_fails_closed_and_reaps_worker() {
    let root = tempfile::tempdir().unwrap();
    let (worker, port) = setup(root.path());
    let transport = WorkerTransport::new(root.path());
    let budget = ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).unwrap();
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
            let event = controller.next_event(&budget).unwrap();
            if event["kind"] == "entry" {
                controller
                    .send(DebugCommand {
                        request_id: 1,
                        phase: "original-control".into(),
                        target: Some(target(&event)),
                        action: DebugAction::SourceNext,
                    })
                    .unwrap();
                break;
            }
        }
        let error = running
            .join()
            .unwrap()
            .err()
            .expect("no assembly-to-source fallback");
        assert_eq!(error.code, "CAPABILITY_UNAVAILABLE");
        let pid = error.details.unwrap()["workerError"]["details"]["pid"]
            .as_i64()
            .unwrap();
        assert_eq!(
            unsafe { libc::kill(pid as i32, 0) },
            -1,
            "worker reaped after unavailable source operation"
        );
    });
}

#[test]
fn step_over_follows_actual_child_listener_and_rejects_stale_stop_token() {
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
        let mut first = None;
        let mut child_seen = false;
        loop {
            let event = controller.next_event(&budget).expect("live event");
            match event["kind"].as_str().expect("event kind") {
                "entry" if event["target"]["invocationIndex"] == 0 => {
                    let token = target(&event);
                    first = Some(token.clone());
                    controller
                        .send(DebugCommand {
                            request_id: 1,
                            phase: "original-control".into(),
                            target: Some(token),
                            action: DebugAction::StepOver,
                        })
                        .expect("step over");
                }
                "entry" => {
                    assert_eq!(event["identity"]["ancestors"], json!([0]));
                    assert_eq!(event["state"], "RUNNING");
                    assert_eq!(target(&event).generation, 1);
                    child_seen = true;
                }
                "stop" if event["target"]["invocationIndex"] == 0 => {
                    assert!(child_seen);
                    assert_eq!(event["stop"]["kind"], "stopped");
                    assert_eq!(target(&event).generation, 1);
                    controller
                        .send(DebugCommand {
                            request_id: 2,
                            phase: "original-control".into(),
                            target: first.take(),
                            action: DebugAction::Registers,
                        })
                        .expect("submit stale token");
                    break;
                }
                _ => {}
            }
        }
        let error = running
            .join()
            .expect("session thread")
            .err()
            .expect("stale access rejected");
        assert_eq!(error.code, "DEBUG_STALE_STATE");
        let pid = error.details.expect("cleanup evidence")["workerError"]["details"]["pid"]
            .as_i64()
            .expect("cancelled worker pid");
        assert_eq!(
            unsafe { libc::kill(pid as i32, 0) },
            -1,
            "worker reaped after routing error"
        );
    });
}
