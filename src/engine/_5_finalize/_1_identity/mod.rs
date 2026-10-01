//! Identity of the actual statically linked Rust coordinator, not its child owner.
use crate::shared::runtime::file_sha256;
use serde_json::{json, Value};
use std::path::PathBuf;
use svm_replay_protocol::{Digest, Error};

mod _0_pin;
use _0_pin as pin;
#[path = "tests.rs"]
#[cfg(test)]
mod tests;

pub(crate) use pin::{publish, CoordinatorPin};
