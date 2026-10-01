#![cfg(target_os = "linux")]
#[path = "../../../_shared/_6_fees/tests/support.rs"]
mod fee_ledger;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, path::Path, time::Duration};
use svm_replay_engine::{
    shared::{
        runtime::{
            file_sha256, CancellationToken, ExecutionBudget, WorkerLimits, WorkerSpec,
            WorkerTransport,
        },
        trace::{CaptureRequest, ProducerBounds},
    },
    {
        _1_resolve_runtime::{ResolvedCaptureWorker, ResolvedWorker},
        _4_simulate::trace::{execute, TraceContext},
    },
};
use svm_replay_protocol::{Digest, PreparedRequest};
#[path = "trace_execution_cases/fixtures.rs"]
mod fixtures;
use fixtures::{fixture, run};

#[path = "trace_execution_cases/fresh_baseline_off_and_trace.rs"]
mod fresh_baseline_off_and_trace;

#[path = "trace_execution_cases/real_historical_jit_capture_matches.rs"]
mod real_historical_jit_capture_matches;
