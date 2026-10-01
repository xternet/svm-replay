//! Additive failure diagnostics; never substitutes state or changes outcomes.
use serde::Serialize;
use serde_json::{json, Value};
use svm_replay_protocol::Error;

mod _0_implementation;
pub(super) use _0_implementation::describe;
