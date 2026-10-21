//! Pinned receipt admission and fresh original-control equality before exposure.
use super::_6_hydrate::SessionHydration;
use super::{NativeSession, SessionCommand};
use crate::shared::runtime::{
    protocol_error, read_bounded_file, ExecutionBudget, WorkerLimits, WorkerOutput, WorkerSpec,
    WorkerTransport,
};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use svm_replay_protocol::{parse_json, Digest, Error, PreparedRequest};
mod _0_types;
use _0_types::fixture_keys;
pub(super) use _0_types::Prepared;
pub use _0_types::{SessionInput, VerifiedSession};
mod _1_artifacts;
mod _2_load;
pub(super) use _2_load::load;
mod _3_open;
mod _4_context;
mod _5_initialized;
mod _6_events;
mod _7_archive;
mod _8_journal;
mod _9_ages;
use _9_ages::AGES;
mod _10_peek;

fn invalid(message: impl Into<String>) -> Error {
    Error::new("SESSION_UNVERIFIED", message)
}
fn text(value: &Value) -> Result<&str, Error> {
    value
        .as_str()
        .ok_or_else(|| invalid("required session evidence string missing"))
}
fn integer(value: &Value) -> Result<u64, Error> {
    value
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| invalid("required safe session evidence integer missing"))
}
fn check(condition: bool, message: &str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(invalid(message))
    }
}
