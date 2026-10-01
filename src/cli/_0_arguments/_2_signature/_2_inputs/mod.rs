use serde_json::Value;
use std::path::Path;
use svm_replay_engine::{shared::runtime::read_bounded_file, TraceOptions};
use svm_replay_protocol::{parse_json, Error};
mod _0_collect;
mod _1_payload;
pub(super) use _0_collect::collect;
use _1_payload::json_argument;
pub(super) use _1_payload::{overrides, replacement};
#[path = "tests/collect.rs"]
#[cfg(test)]
mod collect_tests;
#[path = "tests/tests.rs"]
#[cfg(test)]
mod tests;
