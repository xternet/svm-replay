use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::{fs, path::Path, time::Duration};
use svm_replay_engine::{
    shared::runtime::{
        file_sha256, CancellationToken, ExecutionBudget, WorkerLimits, WorkerTransport,
    },
    {
        _1_resolve_runtime::ResolvedWorker,
        _4_simulate::cache::{execute, CacheContext, CacheOptions},
    },
};
use svm_replay_protocol::Error;
use svm_replay_store::Store;
#[path = "execution_cases/fixtures.rs"]
mod fixtures;
use fixtures::{fixture, run};

#[path = "execution_cases/cold_prepared_hit_and_result.rs"]
mod cold_prepared_hit_and_result;

#[path = "execution_cases/missing_input_is_canonical_and.rs"]
mod missing_input_is_canonical_and;
