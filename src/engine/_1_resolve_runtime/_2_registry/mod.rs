//! Frozen compatibility coverage, never a claim of a unique validator binary.
use serde::Deserialize;
use serde_json::{json, Value};
use svm_replay_protocol::{worker::WorkerDescriptor, Digest, Error};
mod _0_profiles;
use _0_profiles as profiles;
mod _1_names;
use _1_names as names;
#[path = "tests.rs"]
#[cfg(test)]
mod tests;

pub use names::RuntimeName;
pub use profiles::{Profile, Registry, Window};
