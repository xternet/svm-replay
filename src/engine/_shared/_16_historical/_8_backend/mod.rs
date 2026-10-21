//! Private line-delimited JSON backend for an embedding tool (Surfpool): the verified
//! session's own commands on isolated branches, without the public Solana RPC server.
//! Requests and replies are one JSON object per line on the backend's stdin/stdout.
use super::{SessionCommand, VerifiedSession};
use crate::shared::runtime::{protocol_error, ExecutionBudget, WorkerOutput};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use svm_replay_protocol::Error;
mod _0_request;
mod _1_branches;
mod _2_simulate;
mod _3_serve;
mod _4_restore;
mod _5_inventory;
mod _6_archive;
mod _7_journal;
mod _8_resume;
use _0_request::{ArchiveQuery, Op, Request, SortOrder, Status, TokenAccounts};
use _1_branches::{Branches, Commands};
pub use _3_serve::serve_backend;
use _6_archive::Archive;
pub use _7_journal::JournalFile;
use _7_journal::{changes_state, Journal, Journaled};
pub use _8_resume::{recorded_input as recorded_journal_input, require_journal};
use std::path::PathBuf;
#[cfg(test)]
mod tests;
