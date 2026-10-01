use super::*;
use crate::shared::runtime::WorkerSpec;

// Adapter contract only; real worker qualification separately proves observation.
#[test]
fn memory_support_depends_on_qualified_capability_not_runtime_age() {
    for family in ["v2-3", "v3-0", "v3-1", "v4-0", "v4-1"] {
        let pin = Digest::of(b"contract-only worker");
        let descriptor = serde_json::from_value(json!({
            "family":family,"file":"worker","sha256":pin,"buildHash":pin,
            "sourceSha256":pin,"executorSourceId":"contract-test",
            "capabilities":["runtime-full-capture-cli/v1","runtime-call-journal/v1",
                "runtime-sbpf-trace/v1","runtime-interpreter-capture/v1"]
        }))
        .unwrap();
        let reference = ResolvedWorker {
            descriptor,
            spec: WorkerSpec {
                executable: "worker".into(),
                sha256: pin.as_str().into(),
            },
        };
        let mut capture = ResolvedCaptureWorker {
            worker: ResolvedWorker {
                descriptor: reference.descriptor.clone(),
                spec: WorkerSpec {
                    executable: "worker".into(),
                    sha256: pin.as_str().into(),
                },
            },
            reference_sha256: pin.clone(),
            gate_sha256: pin,
        };
        let policy = CaptureRequest::parse(
            &serde_json::to_vec(&json!({
                "schema":"svm-capture-request/v2","executionMode":native_execution_mode(),
                "level":"sbpf","sbpfObservations":"pc-registers-memory",
                "filter":{"programIds":[],"instructionIndices":[]},
                "limits":{"maxBytes":1048576,"maxEvents":1000,"timeoutMs":10000}
            }))
            .unwrap(),
        )
        .unwrap();
        let bounds = ProducerBounds {
            register_rows: 10,
            memory_rows: Some(10),
            max_invocations: 64,
        };
        assert_eq!(
            check_adapter(&capture, &reference, &policy, &bounds)
                .unwrap_err()
                .code,
            "CAPABILITY_UNAVAILABLE"
        );
        capture
            .worker
            .descriptor
            .capabilities
            .push("runtime-sbpf-memory/v1".into());
        check_adapter(&capture, &reference, &policy, &bounds).unwrap();
    }
}
