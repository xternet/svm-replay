//! Bind and validate caller-trusted exact Bank snapshots and guarded input reads.
use crate::shared::diff::{canonical_json, exact_u64};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use svm_replay_protocol::Error;
mod _0_validation;
use _0_validation as validation;

use validation::{
    array, bytes, digest, equal, field, identifier, object, pubkey, require, sha256, shape, slot,
    string, u64_string, Check, EPOCH_FIELDS, INITIALIZED_FIELDS, STAKE,
};
mod _1_provenance;
use _1_provenance as provenance;

use provenance::{
    bound_parts, clock_data, lineage, runtime_matches, snapshot_context, source_and_proof,
};
mod _2_epoch;
use _2_epoch as epoch;

use epoch::epoch_binding;
mod _3_stake;
use _3_stake as stake;

use stake::{initialized_binding, initialized_snapshot, required_stake_set};
mod _4_binding;
use _4_binding as binding;

pub use binding::{
    assert_bound_context, assert_epoch_stake_binding, assert_initialized_stake_binding,
    bind_epoch_stake_evidence, prepare_exact_initialized_stakes,
};
mod _5_failure;
use _5_failure as failure;

pub use failure::validate_epoch_stake_failure;
mod _6_discovery;
use _6_discovery as discovery;

pub use discovery::{available_generic_sysvars, parse_sysvar_discovery_response, GENERIC_SYSVARS};
