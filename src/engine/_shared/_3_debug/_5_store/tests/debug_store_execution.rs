#![cfg(target_os = "linux")]
use serde_json::json;
use std::{fs, path::Path, thread, time::Duration};
use svm_replay_engine::{
    shared::{
        debug::{
            branch_session, channel, open_run, open_session, save_run, save_session, DebugAction,
            DebugCommand, RunArtifacts, SaveOptions,
        },
        diff::canonical_json,
        runtime::{
            file_sha256, read_bounded_file, CancellationToken, ExecutionBudget, ProcessOwner,
            WorkerLimits, WorkerTransport,
        },
        trace::{CaptureRequest, ProducerBounds},
    },
    {
        _1_resolve_runtime::{
            load_catalog, resolve, resolve_capture, ResolvedCaptureWorker, ResolvedWorker,
        },
        _4_simulate::{debug::execute, trace::TraceContext},
    },
};
use svm_replay_protocol::{parse_json, transaction, Digest, PreparedRequest};
#[test]
#[ignore = "requires pinned historical request/catalog/owner, prior real output and authorized artifact root"]
fn real_saved_branch_executes_changed_payer_and_reopens_run_with_root_control() {
    let env = |name: &str| std::env::var(name).unwrap_or_else(|error| panic!("{name}: {error}"));
    let request = PreparedRequest::parse(
        &read_bounded_file(
            Path::new(&env("SVM_REPLAY_TEST_TRACE_REQUEST")),
            256 * 1024 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    let previous = parse_json(
        &read_bounded_file(
            Path::new(&env("SVM_REPLAY_TEST_DEBUG_PRIOR_OUTPUT")),
            256 * 1024 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    let catalog = load_catalog(
        Path::new(&env("SVM_REPLAY_TEST_TRACE_CATALOG")),
        &Digest::new(env("SVM_REPLAY_TEST_TRACE_CATALOG_SHA256")).unwrap(),
    )
    .unwrap();
    let reference = resolve(&catalog, &request).unwrap();
    let capture = resolve_capture(&catalog, &request).unwrap();
    let role = catalog
        .manifest
        .capture_workers
        .iter()
        .find(|role| role.worker.family == request.family)
        .unwrap();
    let owner = ProcessOwner {
        executable: env("SVM_REPLAY_TEST_TRACE_OWNER").into(),
        sha256: env("SVM_REPLAY_TEST_TRACE_OWNER_SHA256"),
    };
    let implementation_binary = std::env::current_exe().unwrap();
    let implementation = Digest::new(file_sha256(&implementation_binary).unwrap()).unwrap();
    let policy=CaptureRequest::parse(&serde_json::to_vec(&json!({"schema":"svm-capture-request/v2","executionMode":"interpreter-debug","level":"sbpf","sbpfObservations":"pc-registers","filter":{"programIds":[],"instructionIndices":[]},"limits":{"maxBytes":request.limits.max_output_bytes,"maxEvents":1000000,"timeoutMs":180000}})).unwrap()).unwrap();
    let bounds = ProducerBounds {
        register_rows: 4,
        memory_rows: None,
        max_invocations: 128,
    };
    let root = tempfile::Builder::new()
        .prefix("debug-saved-history-")
        .tempdir_in(env("SVM_REPLAY_TEST_TRACE_WORK_ROOT"))
        .unwrap()
        .keep();
    eprintln!("saved historical debugger artifacts: {}", root.display());
    let original = root.join("original-session");
    let pin = save_session(
        &original,
        &SaveOptions {
            request: &request,
            reference: &reference.spec,
            reference_descriptor: &reference.descriptor,
            capture: &capture.worker.spec,
            capture_descriptor: role,
            owner: &owner,
            implementation_sha256: &implementation,
            policy: &policy,
            bounds: &bounds,
            symbols: None,
        },
    )
    .unwrap();
    let original = open_session(&original, &pin).unwrap();
    assert_eq!(
        canonical_json(&original.request.fixture),
        canonical_json(&request.fixture)
    );
    let decoded = transaction::decode(
        request.fixture["target"]["transactionBase64"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let payer = decoded["transaction"]["message"]["accountKeys"][0]
        .as_str()
        .unwrap();
    let before = previous["original"]["accountTransitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["pubkey"] == payer)
        .unwrap()["before"]["lamports"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let changed = before.checked_add(1).unwrap();
    let branch = root.join("branch-session");
    let branch_pin = branch_session(
        &original.directory,
        &pin,
        &branch,
        &json!({"requestedAccountOverrides":[{"pubkey":payer,"lamports":changed.to_string()}]}),
    )
    .unwrap();
    let branch = open_session(&branch, &branch_pin).unwrap();
    assert_eq!(
        canonical_json(&branch.control.fixture),
        canonical_json(&request.fixture)
    );
    assert_ne!(
        branch.manifest.session_identity,
        original.manifest.session_identity
    );
    let reference = ResolvedWorker {
        descriptor: branch.manifest.reference.clone(),
        spec: branch.reference.clone(),
    };
    let capture = ResolvedCaptureWorker {
        worker: ResolvedWorker {
            descriptor: branch.manifest.capture.worker.clone(),
            spec: branch.capture.clone(),
        },
        reference_sha256: branch.manifest.capture.reference_sha256.clone(),
        gate_sha256: branch.manifest.capture.gate_sha256.clone(),
    };
    let transport = WorkerTransport::new(&root).with_owner(branch.owner.clone());
    let limits = WorkerLimits {
        max_output_bytes: request.limits.max_output_bytes as usize,
        max_diagnostic_bytes: request.limits.max_diagnostic_bytes as usize,
        ..WorkerLimits::default()
    };
    let budget = ExecutionBudget::new(Duration::from_secs(180), CancellationToken::new()).unwrap();
    let (controller, mut driver) = channel();
    let result = thread::scope(|scope| {
        let running = scope.spawn(|| {
            execute(
                &branch.request,
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
        let mut request_id = 0;
        loop {
            match controller.next_event(&budget) {
                Ok(event) => {
                    if event["kind"] == "session-complete" {
                        break;
                    }
                    if event["kind"] == "entry" && event["state"] == "STOPPED" {
                        request_id += 1;
                        controller
                            .send(DebugCommand {
                                request_id,
                                phase: event["phase"].as_str().unwrap().into(),
                                target: None,
                                action: DebugAction::ContinueAll,
                            })
                            .unwrap();
                    }
                }
                Err(error) => {
                    fs::write(
                        root.join("controller-failure.json"),
                        serde_json::to_vec(&error).unwrap(),
                    )
                    .unwrap();
                    budget.cancel();
                    break;
                }
            }
        }
        running.join().unwrap()
    })
    .unwrap_or_else(|error| {
        fs::write(
            root.join("failure.json"),
            serde_json::to_vec(&error).unwrap(),
        )
        .unwrap();
        panic!("live saved branch: {error}")
    });
    assert_eq!(
        result.control_evidence["fixtureSha256"],
        json!(Digest::of(canonical_json(&request.fixture)))
    );
    assert_eq!(result.output["original"], previous["original"]);
    assert_eq!(result.receipt["debugWorkerCalls"], 2);
    assert_eq!(result.exports.len(), 2);
    let requested = &result.output["requestedOverrides"];
    let transitions = requested["accountTransitions"]
        .as_array()
        .expect("actual requested-account execution");
    let observed = transitions
        .iter()
        .find(|row| row["pubkey"] == payer)
        .unwrap();
    assert_eq!(observed["before"]["lamports"], changed.to_string());
    let run = root.join("saved-run");
    let run_pin = save_run(
        &run,
        &branch,
        &RunArtifacts {
            output: &result.output,
            verification: &result.verification,
            control_evidence: &result.control_evidence,
            events: &result.events,
            receipt: &result.receipt,
            exports: &result.exports,
        },
    )
    .unwrap();
    let reopened = open_run(&run, &run_pin, &branch.pin).unwrap();
    assert_eq!(reopened.output, result.output);
    assert_eq!(reopened.control_evidence, result.control_evidence);
    assert_eq!(reopened.events, result.events);
    assert_eq!(
        file_sha256(&implementation_binary).unwrap(),
        implementation.as_str(),
        "actual coordinating test binary remained pinned"
    );
    fs::write(root.join("receipt.json"),serde_json::to_vec(&json!({"status":"PASS","scope":"real historical saved session reopened; hypothetical payer +1 branch freshly executed; original control preserved; stored run reopened without execution","originalSessionPin":pin,"branchSessionPin":branch_pin,"runManifestSha256":run_pin,"payer":payer,"baselineLamports":before.to_string(),"requestedLamports":changed.to_string(),"debugReceipt":result.receipt,"controlEvidence":result.control_evidence})).unwrap()).unwrap();
}
