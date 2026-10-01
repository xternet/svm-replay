mod _0_config;
use _0_config as config;
mod _1_outcome;
use _1_outcome as outcome;
pub(crate) use outcome::error_receipt;
mod _2_signature;
use _2_signature as signature;
mod _3_wizard;
use _3_wizard as wizard;
use clap::{Args as ClapArgs, CommandFactory, Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::path::PathBuf;
use svm_replay_engine::{
    shared::runtime::{file_sha256, read_bounded_file, ProcessOwner},
    simulate_historical, simulate_prepared, simulate_prepared_with_sources, Config,
};
use svm_replay_engine::{CacheMode, TraceOptions};
use svm_replay_protocol::{parse_json, Digest, Error, HistoricalRequest, Limits, PreparedRequest};

mod _4_types;
#[path = "tests.rs"]
#[cfg(test)]
mod tests;

use _4_types as types;

use types::{Args, BundleCommand, CacheChoice, RuntimeArgs};
mod _5_command;
use _5_command as command;

use command::Command;
mod _6_entry;
use _6_entry as entry;

pub use entry::run;

mod _7_dispatch;
use _7_dispatch as dispatch;

use dispatch::show_help;
mod _8_execute;
use _8_execute as execute;

use execute::execute;
mod _10_capture;
use _10_capture as capture;

use capture::captured_pair;
