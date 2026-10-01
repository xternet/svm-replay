//! Exact historical input access; absence is evidence, unavailability is an error.
use super::{
    runtime::ExecutionBudget,
    sources::{CompositeSource, SourceError},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use svm_replay_protocol::{transaction, Digest, Error};
mod _0_history;
use _0_history as history;

pub use history::{
    account_data, address, classify_account, data, program_data_address, History, Roles, CLOCK,
    INSTRUCTIONS, RECENT_BLOCKHASHES, UPGRADEABLE_LOADER,
};

use history::invalid;
mod _1_history_validation;
use _1_history_validation as history_validation;

pub use history_validation::source_error;
