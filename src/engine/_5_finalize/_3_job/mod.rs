//! Per-job lifecycle records. Execution orchestration stays in the workflow root.
use super::{ensure_private_directory, write_json};
use crate::Config;
use crate::_5_finalize::identity::CoordinatorPin;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use svm_replay_protocol::{Digest, Error};

mod _0_receipt;
mod _1_incomplete;
use _0_receipt as receipt;
#[path = "tests.rs"]
#[cfg(test)]
mod tests;

pub(crate) use receipt::{begin_job, finish_job};
