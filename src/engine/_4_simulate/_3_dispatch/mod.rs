//! Select ordinary, checkpointed or fresh observation execution for one attempt.
use super::worker_error;
use crate::_5_finalize::identity::CoordinatorPin;
use crate::shared::runtime::{ExecutionBudget, WorkerLimits, WorkerTransport};
use crate::{
    CacheMode, Config, _1_resolve_runtime, _2_prepare_state, _3_verify_original, _4_simulate,
    _5_finalize,
};
use serde_json::{json, Value};
use std::time::Duration;
use svm_replay_protocol::{Error, MetadataPolicy, PreparedRequest};
mod _0_execution;
use _0_execution as execution;

pub(crate) use execution::execute;
