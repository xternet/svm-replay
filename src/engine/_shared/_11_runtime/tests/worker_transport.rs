#![cfg(target_os = "linux")]

use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use svm_replay_engine::shared::runtime::{
    CancellationToken, ExecutionBudget, WorkerErrorCode, WorkerLimits, WorkerSpec, WorkerTransport,
};
#[path = "worker_transport_cases/fixtures.rs"]
mod fixtures;
use fixtures::{budget, limits, worker};

#[path = "worker_transport_cases/live_diagnostics_are_bounded_and.rs"]
mod live_diagnostics_are_bounded_and;

#[path = "worker_transport_cases/timeout_and_cancellation_kill_and.rs"]
mod timeout_and_cancellation_kill_and;
