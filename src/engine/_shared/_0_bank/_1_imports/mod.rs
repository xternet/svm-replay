//! Conservative executable import inventory. Dynamic generic reads still need a worker guard.
use crate::shared::history::{account_data, program_data_address};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use svm_replay_protocol::{Digest, Error};
mod _0_inspection;
use _0_inspection as inspection;

pub use inspection::{dedicated_inputs, inspect};
