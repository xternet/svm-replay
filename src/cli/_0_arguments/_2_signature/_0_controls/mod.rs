//! Optional signature controls; defaults remain one command, not a prepared request.
use clap::Args;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use svm_replay_engine::{
    _1_resolve_runtime::registry::Registry, shared::runtime::read_bounded_file,
};
use svm_replay_protocol::{Digest, Error, Limits};

#[path = "tests.rs"]
#[cfg(test)]
mod tests;

mod _0_implementation;

pub(super) use _0_implementation::{remaining, Controls};
