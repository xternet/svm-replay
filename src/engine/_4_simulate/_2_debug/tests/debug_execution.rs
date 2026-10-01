#![cfg(target_os = "linux")]
use serde_json::json;
use std::{fs, path::Path, thread, time::Duration};
use svm_replay_engine::{
    shared::{
        debug::{channel, DebugAction, DebugCommand, PauseToken},
        runtime::{
            file_sha256, read_bounded_file, CancellationToken, ExecutionBudget, ProcessOwner,
            WorkerLimits, WorkerTransport,
        },
        trace::{CaptureRequest, ProducerBounds},
    },
    {
        _1_resolve_runtime::{load_catalog, resolve, resolve_capture},
        _4_simulate::{debug::execute, trace::TraceContext},
    },
};
use svm_replay_protocol::{Digest, PreparedRequest};

#[test]
#[ignore = "requires pinned historical request/catalog/owner and approved artifact root"]
fn real_historical_live_step_register_read_and_resume_have_jit_parity() {
    let env =
        |name: &str| std::env::var(name).unwrap_or_else(|error| panic!("{name} required: {error}"));
    let request_path = env("SVM_REPLAY_TEST_TRACE_REQUEST");
    let catalog_path = env("SVM_REPLAY_TEST_TRACE_CATALOG");
    let catalog_pin = Digest::new(env("SVM_REPLAY_TEST_TRACE_CATALOG_SHA256")).unwrap();
    let mut request = PreparedRequest::parse(
        &read_bounded_file(Path::new(&request_path), 256 * 1024 * 1024).unwrap(),
    )
    .unwrap();
    let catalog = load_catalog(Path::new(&catalog_path), &catalog_pin).unwrap();
    let reference = resolve(&catalog, &request).unwrap();
    let capture = resolve_capture(&catalog, &request).unwrap();
    let root = tempfile::Builder::new()
        .prefix("debug-history-")
        .tempdir_in(env("SVM_REPLAY_TEST_TRACE_WORK_ROOT"))
        .unwrap()
        .keep();
    eprintln!("historical live debugger artifacts: {}", root.display());
    let inputs =
        svm_replay_engine::_2_prepare_state::discovery::initial_inputs(&request.fixture).unwrap();
    request.fixture =
        svm_replay_engine::_2_prepare_state::discovery::tracked_fixture(&request.fixture, &inputs)
            .unwrap();
    fs::write(
        root.join("prepared-request.json"),
        serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    let owner = ProcessOwner {
        executable: env("SVM_REPLAY_TEST_TRACE_OWNER").into(),
        sha256: env("SVM_REPLAY_TEST_TRACE_OWNER_SHA256"),
    };
    let transport = WorkerTransport::new(&root).with_owner(owner);
    let implementation =
        Digest::new(file_sha256(&std::env::current_exe().unwrap()).unwrap()).unwrap();
    let policy=CaptureRequest::parse(&serde_json::to_vec(&json!({"schema":"svm-capture-request/v2","executionMode":"interpreter-debug","level":"sbpf","sbpfObservations":"pc-registers","filter":{"programIds":[],"instructionIndices":[]},"limits":{"maxBytes":request.limits.max_output_bytes,"maxEvents":1000000,"timeoutMs":180000}})).unwrap()).unwrap();
    let bounds = ProducerBounds {
        register_rows: 4,
        memory_rows: None,
        max_invocations: 128,
    };
    let limits = WorkerLimits {
        max_output_bytes: request.limits.max_output_bytes as usize,
        max_diagnostic_bytes: request.limits.max_diagnostic_bytes as usize,
        ..WorkerLimits::default()
    };
    let budget = ExecutionBudget::new(Duration::from_secs(180), CancellationToken::new()).unwrap();
    let (controller, mut driver) = channel();
    let result = thread::scope(|scope| {
        let replay = scope.spawn(|| {
            execute(
                &request,
                &reference,
                &capture,
                TraceContext {
                    transport: &transport,
                    scratch_root: &root,
                    implementation_sha256: &implementation,
                },
                &policy,
                &bounds,
                &limits,
                &budget,
                &mut driver,
            )
        });
        let mut phase = "original-control".to_owned();
        let mut selected: Option<PauseToken> = None;
        let mut before = None;
        let mut after = None;
        let mut delivered = Vec::new();
        let mut request_id = 0;
        let mut interaction_error = None;
        loop {
            let event = match controller.next_event(&budget) {
                Ok(value) => value,
                Err(error) => {
                    interaction_error = Some(error);
                    break;
                }
            };
            delivered.push(event.clone());
            if event["kind"] == "session-complete" {
                break;
            }
            if event["kind"] == "phase-start" {
                phase = event["phase"].as_str().unwrap().into();
            }
            let mut command = None;
            if event["kind"] == "entry" && event["state"] == "STOPPED" {
                let token: PauseToken = serde_json::from_value(event["target"].clone()).unwrap();
                if selected.is_none() {
                    selected = Some(token.clone());
                    command = Some((Some(token), DebugAction::Registers));
                } else {
                    command = Some((Some(token), DebugAction::Continue));
                }
            }
            if event["kind"] == "response" && event["result"].get("registers").is_some() {
                if before.is_none() {
                    before = Some(event["result"]["registers"].clone());
                    command = Some((selected.clone(), DebugAction::StepOver));
                } else {
                    after = Some(event["result"]["registers"].clone());
                    command = Some((None, DebugAction::ContinueAll));
                }
            }
            if event["kind"] == "stop" && event["stop"]["kind"] == "stopped" {
                let token: PauseToken = serde_json::from_value(event["target"].clone()).unwrap();
                if selected.as_ref().is_some_and(|vm| {
                    vm.execution_index == token.execution_index
                        && vm.invocation_index == token.invocation_index
                }) && after.is_none()
                {
                    selected = Some(token.clone());
                    command = Some((Some(token), DebugAction::Registers));
                } else {
                    command = Some((Some(token), DebugAction::Continue));
                }
            }
            if let Some((target, action)) = command {
                request_id += 1;
                if let Err(error) = controller.send(DebugCommand {
                    request_id,
                    phase: phase.clone(),
                    target,
                    action,
                }) {
                    interaction_error = Some(error);
                    budget.cancel();
                    break;
                }
            }
        }
        fs::write(
            root.join("delivered-events.json"),
            serde_json::to_vec(&delivered).unwrap(),
        )
        .unwrap();
        let result = replay.join().expect("bounded debugger coordinator");
        match result {
            Ok(result) => {
                assert!(
                    interaction_error.is_none(),
                    "controller: {interaction_error:?}"
                );
                let before = before.expect("actual pre-step registers");
                let after = after.expect("actual post-step registers");
                assert_ne!(
                    before[11], after[11],
                    "actual PC must advance after one SBPF step"
                );
                Ok(result)
            }
            Err(error) => {
                fs::write(
                    root.join("failure.json"),
                    serde_json::to_vec(
                        &json!({"coordinator":error,"controller":interaction_error}),
                    )
                    .unwrap(),
                )
                .unwrap();
                Err(error)
            }
        }
    })
    .unwrap_or_else(|error| panic!("historical debugger failed: {error}"));
    fs::write(
        root.join("receipt.json"),
        serde_json::to_vec(&result.receipt).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("events.json"),
        serde_json::to_vec(&result.events).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("output.json"),
        serde_json::to_vec(&result.output).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("verification.json"),
        serde_json::to_vec(&result.verification).unwrap(),
    )
    .unwrap();
    fs::write(root.join("identity.json"),serde_json::to_vec(&json!({"requestPath":request_path,"catalogPath":catalog_path,"catalogSha256":catalog_pin,"implementationSha256":implementation,"reference":reference.descriptor,"capture":capture.worker.descriptor,"gateSha256":capture.gate_sha256,"policy":policy,"bounds":bounds})).unwrap()).unwrap();
    for (phase, export) in result.exports {
        fs::write(
            root.join(format!("{phase}-capture-events.json")),
            export.payload,
        )
        .unwrap();
        fs::write(
            root.join(format!("{phase}-artifact.json")),
            serde_json::to_vec(&export.artifact).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(result.receipt["status"], "PASS");
    eprintln!(
        "historical live debugger: exact JIT/interpreter parity; {} live events",
        result.events.len()
    );
}
