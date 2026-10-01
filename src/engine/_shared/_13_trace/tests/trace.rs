use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use svm_replay_engine::shared::trace::{
    capture_identity, make_export, validate_artifact, validate_capability, CaptureRequest,
    ProducerBounds,
};
use svm_replay_protocol::{worker::WorkerDescriptor, Digest};
#[path = "trace_cases/fixtures.rs"]
mod fixtures;
use fixtures::{bounds, journal, output, policy, request, super_policy, worker};

#[path = "trace_cases/capture_request_is_strict_and.rs"]
mod capture_request_is_strict_and;

#[path = "trace_cases/truncation_partial_memory_and_payload.rs"]
mod truncation_partial_memory_and_payload;
