use super::*;

pub(super) fn policy() -> Value {
    json!({"schema":"svm-capture-request/v2","executionMode":"jit","level":"calls","sbpfObservations":"none",
        "filter":{"programIds":[],"instructionIndices":[]},"limits":{"maxBytes":1048576,"maxEvents":100,"timeoutMs":1000}})
}

pub(super) fn request(value: &Value) -> CaptureRequest {
    CaptureRequest::parse(&serde_json::to_vec(value).expect("request bytes"))
        .expect("valid request")
}

pub(super) fn worker() -> WorkerDescriptor {
    serde_json::from_value(json!({"family":"v4-2","file":"capture-worker","sha256":Digest::of(b"synthetic worker"),
        "buildHash":Digest::of(b"synthetic build"),"sourceSha256":Digest::of(b"synthetic source"),"executorSourceId":"synthetic-capture",
        "capabilities":["runtime-call-journal/v1","runtime-sbpf-trace/v1","runtime-sbpf-memory/v1","runtime-interpreter-debug/v1"]}))
        .expect("protocol descriptor")
}

pub(super) fn bounds() -> ProducerBounds {
    ProducerBounds {
        register_rows: 0,
        memory_rows: None,
        max_invocations: 64,
    }
}

pub(super) fn output(events: Vec<Value>) -> Value {
    json!({"schema":"svm-historical-capture-worker-experimental/v1","status":"COMPLETE","execution_mode":"jit","captures":[],
        "journals":[{"signatures":[],"scope":"target-and-variants","before_scope":"tracked accounts","before_keys":[],
            "terminal_scope":"committed tracked accounts","terminal_keys":[],"journal":{"disposition":"COMPLETE","reason":null,
                "event_count":events.len(),"payload_json":serde_json::to_string(&events).expect("journal")}}]})
}

pub(super) fn journal() -> Vec<Value> {
    vec![
        json!({"kind":"transaction_begin","accounts":[]}),
        json!({"kind":"transaction_commit","outcome":{"status":"success"},"accounts":[]}),
    ]
}

pub(super) fn super_policy() -> Value {
    policy()
}
