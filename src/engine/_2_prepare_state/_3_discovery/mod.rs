//! Exact generic-sysvar inputs are accumulated; each execution uses a fresh fixture.
use super::context;
use crate::shared::{
    bank::{
        context::historical_bank_context_hash,
        slot_hashes,
        sysvars::{
            assert_bank_initialized_sysvar_bindings, bind_exact_generic_sysvar, GENERIC_SYSVARS,
        },
    },
    diff::canonical_json,
    history::CLOCK,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use svm_replay_protocol::{Digest, Error};
mod _0_discovery;
use _0_discovery as discovery;

use discovery::{invalid, target_slot};

pub use discovery::{add_input, initial_inputs, runtime_profile_hash, tracked_fixture, Settled};
mod _1_settle;
use _1_settle as settle;

pub use settle::settle;
