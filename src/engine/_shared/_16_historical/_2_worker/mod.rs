//! Stateful counterfactual command transport, not standard Solana RPC admission.
//! Historical preparation and original-control proof stay upstream.
use crate::shared::runtime::{
    protocol_error, ExecutionBudget, WorkerError, WorkerLimits, WorkerOutput, WorkerSpec,
    WorkerTransport,
};
use serde_json::Value;
use svm_replay_protocol::Error;

mod _0_command;
pub use _0_command::SessionCommand;
#[cfg(unix)]
mod _1_frames;
#[cfg(unix)]
mod _2_start;
#[cfg(unix)]
mod _3_lifecycle;
#[cfg(not(unix))]
mod _4_unsupported;

#[cfg(unix)]
pub struct NativeSession {
    wire: Option<_1_frames::Wire>,
    supervisor: Option<std::thread::JoinHandle<Result<WorkerOutput, WorkerError>>>,
    ipc: Option<tempfile::TempDir>,
    budget: ExecutionBudget,
    ready: Value,
    next_id: u64,
}
#[cfg(not(unix))]
pub use _4_unsupported::NativeSession;

fn error(message: impl Into<String>) -> Error {
    Error::new("SESSION_PROTOCOL", message)
}
