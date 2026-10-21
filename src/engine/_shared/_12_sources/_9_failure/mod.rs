//! Provider attribution changes diagnostics only, never error codes or replay state.
use super::*;
use svm_replay_protocol::Error;
mod _0_implementation;
pub(crate) use _0_implementation::{attribute_failure, attribute_source_failure};
#[cfg(test)]
mod tests;
