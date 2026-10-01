//! Coordinate verified prepared caches inside the execution workflow.
use crate::_5_finalize::identity::{publish, CoordinatorPin};
use crate::shared::cache::{
    boundary::{self, checkpoint_response, validate_checkpoint, CheckpointMode, Metrics},
    identity,
};
use crate::shared::{
    diff::canonical_json,
    runtime::{
        file_sha256, protocol_error, read_bounded_file, ExecutionBudget, WorkerLimits,
        WorkerTransport,
    },
};
use crate::{_1_resolve_runtime::ResolvedWorker, _2_prepare_state, _3_verify_original};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use svm_replay_protocol::{parse_json, Digest, Error, MetadataPolicy, PreparedRequest};
use svm_replay_store::Store;
mod _0_storage;
use _0_storage as storage;
pub use storage::{execute, CacheContext, CacheOptions};
use storage::{
    field, get, put, response_receipt, store_error, uint, verify, verify_worker, write_checkpoint,
};
mod _1_execute_pinned;
use _1_execute_pinned as execute_pinned;
pub(crate) use execute_pinned::execute_pinned;
mod _2_execute_inner;
use _2_execute_inner as execute_inner;
use execute_inner::execute_inner;
mod _3_admit;
use _3_admit as admit;
use admit::admit;
mod _4_checkpoint_run;
use _4_checkpoint_run as checkpoint_run;
use checkpoint_run::checkpoint_run;
mod _5_attempt_lease;
use _5_attempt_lease as attempt_lease;
use attempt_lease::attempt_lease;
mod _6_admit_cached_result;
use _6_admit_cached_result as admit_cached_result;
use admit_cached_result::admit_cached_result;
