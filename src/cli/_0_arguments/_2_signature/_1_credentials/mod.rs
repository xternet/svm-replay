//! Load only API_ALCHEMY; never execute shell syntax or import other variables.
use std::path::Path;
use svm_replay_engine::shared::runtime::read_bounded_file;
use svm_replay_protocol::Error;

mod _0_implementation;

pub(super) use _0_implementation::{load, load_dotenv};
