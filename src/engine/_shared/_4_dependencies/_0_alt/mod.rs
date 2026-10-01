//! Canonical parent ALT preflight proofs; runtime membership still uses exact evidence.
use super::{
    lifecycle::{unsupported, ALT},
    Result,
};
use crate::shared::history::account_data;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use svm_replay_protocol::{Digest, Error};
mod _0_proof;
use _0_proof as proof;

pub use proof::{build_recent_address_table_proofs, preflight_active_address_tables};
