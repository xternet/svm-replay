use super::*;

#[test]
#[ignore = "requires externally pinned historical request/catalog/owner and approved artifact root"]
fn real_historical_jit_capture_matches_fresh_reference_and_off() {
    use svm_replay_engine::_1_resolve_runtime::{load_catalog, resolve, resolve_capture};
    use svm_replay_engine::shared::runtime::{read_bounded_file, ProcessOwner};
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
        .prefix("trace-history-")
        .tempdir_in(env("SVM_REPLAY_TEST_TRACE_WORK_ROOT"))
        .unwrap()
        .keep();
    eprintln!("historical trace artifacts: {}", root.display());
    // Match the root's explicit exact-Clock preflight; retain the derived input
    // rather than altering the preserved historical request in place.
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
    let policy=CaptureRequest::parse(&serde_json::to_vec(&json!({"schema":"svm-capture-request/v2","executionMode":"jit","level":"sbpf","sbpfObservations":"pc-registers",
        "filter":{"programIds":[],"instructionIndices":[]},"limits":{"maxBytes":request.limits.max_output_bytes,"maxEvents":1000000,"timeoutMs":180000}})).unwrap()).unwrap();
    let limits = WorkerLimits {
        max_output_bytes: request.limits.max_output_bytes as usize,
        max_diagnostic_bytes: request.limits.max_diagnostic_bytes as usize,
        ..WorkerLimits::default()
    };
    let result = execute(
        &request,
        &reference,
        &capture,
        TraceContext {
            transport: &transport,
            scratch_root: &root,
            implementation_sha256: &implementation,
        },
        &policy,
        &ProducerBounds {
            register_rows: 4,
            memory_rows: None,
            max_invocations: 128,
        },
        &limits,
        &ExecutionBudget::new(Duration::from_millis(180000), CancellationToken::new()).unwrap(),
    );
    let executed = match result {
        Ok(executed) => executed,
        Err(error) => {
            fs::write(
                root.join("failure.json"),
                serde_json::to_vec(&error).unwrap(),
            )
            .unwrap();
            panic!("historical trace failed: {error}");
        }
    };
    fs::write(
        root.join("receipt.json"),
        serde_json::to_vec(&executed.receipt).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("output.json"),
        serde_json::to_vec(&executed.output).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("verification.json"),
        serde_json::to_vec(&executed.verification).unwrap(),
    )
    .unwrap();
    fs::write(root.join("identity.json"),serde_json::to_vec(&json!({"requestPath":request_path,"requestSha256":Digest::of(read_bounded_file(Path::new(&request_path),256*1024*1024).unwrap()),
        "catalogPath":catalog_path,"catalogSha256":catalog_pin,"implementationSha256":implementation,"reference":reference.descriptor,"capture":capture.worker.descriptor,
        "gateSha256":capture.gate_sha256,"policy":policy})).unwrap()).unwrap();
    let mut registers = 0;
    for (phase, export) in &executed.exports {
        fs::write(root.join(format!("{phase}-events.json")), &export.payload).unwrap();
        fs::write(
            root.join(format!("{phase}-artifact.json")),
            serde_json::to_vec(&export.artifact).unwrap(),
        )
        .unwrap();
        let events = svm_replay_protocol::parse_json(&export.payload).unwrap();
        registers += events
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["kind"] == "registers")
            .count();
        assert!(["COMPLETE", "TRUNCATED", "PARTIAL"]
            .contains(&export.artifact["status"].as_str().unwrap()));
    }
    assert!(
        registers > 0,
        "selected historical SBPF target must yield actual register rows"
    );
    assert_eq!(executed.receipt["status"], "PASS");
    eprintln!(
        "historical trace: {} worker calls; {registers} bounded register rows",
        executed.receipt["workerCalls"]
    );
}
