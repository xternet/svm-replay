//! One job: validate, prepare, run original control, simulate and save evidence.
pub mod _0_validate;
pub mod _1_resolve_runtime;
pub mod _2_prepare_state;
pub mod _3_verify_original;
pub mod _4_simulate;
pub mod _5_finalize;
pub mod _shared;
use _2_prepare_state::source::hydrate::hydrate_exact;
pub use _4_simulate::live::debug_prepared;
use _4_simulate::{
    retry::{execute, execute_hydrated},
    worker_error,
};
use _5_finalize::{
    job::{begin_job, finish_job},
    sources::record_sources,
};
pub use _shared as shared;
use serde_json::{json, Value};
pub use shared::config::{CacheMode, Config, TraceOptions};
use shared::{
    history::History,
    runtime::{CancellationToken, ExecutionBudget},
    sources::CompositeSource,
};
use std::time::{Duration, Instant};
pub use svm_replay_protocol::{Error, HistoricalRequest, PreparedRequest};

mod _6_workflow;
#[path = "tests.rs"]
#[cfg(test)]
mod control_evidence_tests;

use _6_workflow as workflow;

pub use workflow::{simulate_historical, simulate_prepared, simulate_prepared_with_sources};
