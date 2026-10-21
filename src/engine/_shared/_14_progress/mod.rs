//! Opt-in progress scoped to the calling coordinator thread. No receipt changes.
mod _0_schema;
mod _1_state;
mod _2_job;
mod _3_updates;
pub use _0_schema::{Kind, Phase, ProgressEvent, Total, TotalStatus};
use _1_state::State;
pub use _2_job::{ProgressJob, Sink};
pub use _3_updates::{
    account_fetched, attempt, cache_hit, download_unknown, downloaded, earlier_total, phase,
    processed_earlier, transport_started,
};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
thread_local! {
    static CURRENT: RefCell<Option<Arc<_2_job::Shared>>> = const { RefCell::new(None) };
}
#[cfg(test)]
mod tests;
