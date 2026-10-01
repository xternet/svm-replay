#![cfg(target_os = "linux")]
use serde_json::{json, Value};
use std::{fs, net::TcpListener, os::unix::fs::PermissionsExt, thread, time::Duration};
use svm_replay_engine::shared::{
    debug::{channel, run_session, DebugAction, DebugCommand, PauseToken},
    runtime::{
        file_sha256, CancellationToken, ExecutionBudget, WorkerLimits, WorkerSpec, WorkerTransport,
    },
};
#[path = "debug_session_cases/fixtures.rs"]
mod fixtures;
use fixtures::{setup, target, WORKER};

#[path = "debug_session_cases/source_command_without_admitted_symbols.rs"]
mod source_command_without_admitted_symbols;

#[path = "debug_session_cases/step_out_returns_to_real.rs"]
mod step_out_returns_to_real;
