use super::InitializationRequest;
use super::{SessionCommand, SessionInput, _3_proof};
use crate::{
    shared::{
        runtime::{file_sha256, protocol_error, ExecutionBudget},
        sources::CompositeSource,
    },
    Config,
};
use serde_json::Value;
use svm_replay_protocol::{Error, HistoricalRequest};
#[derive(Clone)]
enum CanonicalRequest {
    Transaction(HistoricalRequest),
    Initialization(InitializationRequest),
}
mod _0_state;
pub use _0_state::SessionHydration;
mod _1_expand;
mod _2_archive;
mod _3_journal;
