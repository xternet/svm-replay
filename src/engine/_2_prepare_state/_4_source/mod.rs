//! Source acquisition -> causal closure -> exact pre-transaction images. No execution here.
use super::{analysis::analyze, discovery::runtime_profile_hash};
mod _0_context;
use _0_context as context;
pub(crate) mod _1_hydrate;
pub(crate) use _1_hydrate as hydrate;
pub mod _2_signature;
use crate::shared::{
    bank::{context as bank_context, imports, migration, restart, snapshots, stake, sysvars},
    dependencies::{self, alt, lifecycle, override_sysvars, overrides, requested},
    diff::canonical_json,
    fees,
    history::{classify_account, History, Roles, CLOCK, INSTRUCTIONS, RECENT_BLOCKHASHES},
    instructions,
};
pub use _2_signature as signature;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use svm_replay_protocol::{Digest, Error, HistoricalRequest, PreparedRequest};
mod _3_source;
use _3_source as source;

use source::{array, invalid, required, strings, unique, RENT, RESTART};

pub use source::Prepared;
mod _4_reconstruct;
use _4_reconstruct as reconstruct;

pub use reconstruct::reconstruct;

mod _5_bank_inputs;
use _5_bank_inputs as bank_inputs;
use bank_inputs::bank_inputs;

mod _6_runtime_requirements;
use _6_runtime_requirements as runtime_requirements;
use runtime_requirements::runtime_requirements;

mod _7_parent_keys;
use _7_parent_keys as parent_keys;
use parent_keys::parent_keys;

mod _8_account_coverage;
use _8_account_coverage::assert_account_coverage;
mod _10_credit_recovery;
mod _11_program_headers;
mod _9_implicit_loaders;
