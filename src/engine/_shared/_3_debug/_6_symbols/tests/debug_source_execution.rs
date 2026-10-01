#![cfg(target_os = "linux")]
use serde_json::{json, Value};
use std::{fs, net::TcpListener, os::unix::fs::PermissionsExt, sync::mpsc, thread, time::Duration};
use svm_replay_engine::shared::{
    debug::{DebugClient, ExactSymbols, SourceNavigation, SourceNavigationKind, Stop},
    runtime::{
        file_sha256, read_bounded_file, CancellationToken, ExecutionBudget, ProcessOwner,
        WorkerLimits, WorkerSpec, WorkerTransport,
    },
};
use svm_replay_protocol::{parse_json, Digest};
#[path = "debug_source_execution_cases/fixtures.rs"]
mod fixtures;
use fixtures::{nav, ADAPTER};

#[path = "debug_source_execution_cases/real_sbpf_source_variables_frames.rs"]
mod real_sbpf_source_variables_frames;
