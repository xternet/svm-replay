//! Fresh reference/off/enabled parity under one unchanged prepared context.
use crate::{
    shared::{
        diff::canonical_json,
        runtime::{
            file_sha256, protocol_error, read_bounded_file, ExecutionBudget, WorkerLimits,
            WorkerTransport,
        },
        trace::{
            capture_identity, make_export, validate_capability, CaptureExport, CaptureRequest,
            ProducerBounds,
        },
    },
    {
        _1_resolve_runtime::{validate_worker_binding, ResolvedCaptureWorker, ResolvedWorker},
        _2_prepare_state, _3_verify_original,
    },
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
use svm_replay_protocol::{Digest, Error, MetadataPolicy, PreparedRequest};
mod _0_trace;
use _0_trace as trace;

pub use trace::{TraceContext, TraceExecution};

use trace::{check_adapter, exact, problem, verify_pin};

pub(super) use trace::{native_execution_mode, payload, verify};
mod _1_execute;
use _1_execute as execute;

pub use execute::execute;

#[cfg(test)]
mod tests;
