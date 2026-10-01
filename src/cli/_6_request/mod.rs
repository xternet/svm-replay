use serde_json::{json, Value};
use std::{fs, io::Write, path::Path};
use svm_replay_engine::shared::runtime::read_bounded_file;
use svm_replay_protocol::{
    parse_json, Digest, Error, HistoricalRequest, Limits, MetadataPolicy, PreparedRequest,
};

#[path = "tests.rs"]
#[cfg(test)]
mod tests;

pub use svm_replay_engine::_2_prepare_state::boundary::boundary;
mod _0_builder;
use _0_builder as builder;

pub use builder::{build, inspect, read, save, save_bytes};
