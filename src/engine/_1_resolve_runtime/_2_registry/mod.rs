//! Frozen compatibility coverage, never a claim of a unique validator binary.
use serde::Deserialize;
use serde_json::{json, Value};
use svm_replay_protocol::{worker::WorkerDescriptor, Digest, Error};
mod _0_profiles;
use _0_profiles as profiles;
#[path = "tests.rs"]
#[cfg(test)]
mod tests;

pub use profiles::{Profile, Registry, Window};
