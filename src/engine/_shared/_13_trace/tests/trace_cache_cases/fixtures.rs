use super::*;

pub(super) fn identity() -> ExportCacheIdentity {
    let request = CaptureRequest::parse(
        &serde_json::to_vec(&json!({
            "schema":"svm-capture-request/v2","executionMode":"jit","level":"calls",
            "sbpfObservations":"none","filter":{"programIds":[],"instructionIndices":[]},
            "limits":{"maxBytes":1048576,"maxEvents":1,"timeoutMs":1000}
        }))
        .unwrap(),
    )
    .unwrap();
    let worker: WorkerDescriptor = serde_json::from_value(json!({
        "family":"synthetic","file":"worker","sha256":Digest::of(b"worker"),
        "buildHash":Digest::of(b"build"),"sourceSha256":Digest::of(b"source"),
        "executorSourceId":"test-only","capabilities":["runtime-call-journal/v1","runtime-sbpf-trace/v1"]
    })).unwrap();
    let bounds = ProducerBounds {
        register_rows: 0,
        memory_rows: None,
        max_invocations: 8,
    };
    let input = Digest::of(b"explicit synthetic input");
    let artifact_identity = capture_identity(&input, &worker, &request, &bounds).unwrap();
    ExportCacheIdentity {
        input_sha256: input,
        worker,
        request,
        producer_bounds: bounds,
        context_sha256: Digest::of(b"explicit synthetic verified context"),
        artifact_identity_sha256: artifact_identity,
    }
}

pub(super) fn export(id: &ExportCacheIdentity) -> svm_replay_engine::shared::trace::CaptureExport {
    let events = json!([{"kind":"transaction_begin","accounts":[]},
        {"kind":"transaction_commit","outcome":{"status":"success"},"accounts":[]}]);
    let output = json!({"schema":"svm-historical-capture-worker-experimental/v1","status":"COMPLETE",
        "execution_mode":"jit","captures":[],"journals":[{"signatures":[],"scope":"target-and-variants",
        "before_scope":"tracked accounts","before_keys":[],"terminal_scope":"committed tracked accounts",
        "terminal_keys":[],"journal":{"disposition":"COMPLETE","reason":null,"event_count":2,
        "payload_json":serde_json::to_string(&events).unwrap()}}]});
    make_export(&id.request, &id.artifact_identity_sha256, &output).unwrap()
}

pub(super) fn raw_get(store: &mut Store, key: &Digest) -> Vec<u8> {
    let response = store
        .execute(json!({"version":1,"op":"get","namespace":"raw","key":key}))
        .unwrap();
    assert_eq!(response["status"], "HIT");
    STANDARD
        .decode(response["dataBase64"].as_str().unwrap())
        .unwrap()
}

pub(super) fn raw_put(store: &mut Store, key: &Digest, bytes: &[u8]) {
    store
        .execute(json!({"version":1,"op":"put","namespace":"raw","key":key,
        "dataBase64":STANDARD.encode(bytes)}))
        .unwrap();
}
