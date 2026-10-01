//! Publish immutable job outputs after all worker ownership has been released.
pub(crate) mod _0_attempts;
pub(crate) use _0_attempts as attempts;
pub(crate) mod _1_identity;
pub(crate) use _1_identity as identity;
pub(crate) mod _2_sources;
pub(crate) use _2_sources as sources;
use serde_json::Value;
use std::{fs::File, io::Write, path::Path};
use svm_replay_protocol::{Digest, Error};
pub(crate) mod _3_job;
pub(crate) use _3_job as job;
mod _4_receipt;
use _4_receipt as receipt;

pub use receipt::{ensure_private_directory, inspect_job, write_bytes, write_json};
