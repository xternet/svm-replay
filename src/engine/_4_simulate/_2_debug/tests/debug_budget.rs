#![cfg(target_os = "linux")]
#[path = "../../../_shared/_6_fees/tests/support.rs"]
mod fee_ledger;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use svm_replay_engine::{
    shared::{
        debug::channel,
        runtime::{
            file_sha256, CancellationToken, ExecutionBudget, WorkerLimits, WorkerSpec,
            WorkerTransport,
        },
        trace::{CaptureRequest, ProducerBounds},
    },
    {
        _1_resolve_runtime::{ResolvedCaptureWorker, ResolvedWorker},
        _4_simulate::{debug::execute, trace::TraceContext},
    },
};
use svm_replay_protocol::{Digest, PreparedRequest};
#[path = "debug_budget_cases/fixtures.rs"]
mod fixtures;
use fixtures::run;

#[path = "debug_budget_cases/jit_gate_time_does_not.rs"]
mod jit_gate_time_does_not;
