//! Requested ALT boundary resolution. No original-state mutation or live provider adapter.
use super::{
    overrides::{bytes, checked_overrides, invalid, u32_at, u64_at, RequestedAccountOverride},
    safe_integer, summarize_semantic_transaction, unique, Result, SemanticTransaction,
};
use crate::shared::history::{History, Roles};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use svm_replay_protocol::{transaction::decode, Error};
mod _0_instructions;
use _0_instructions as instructions;

use instructions::{
    array, index, object, pubkeys, recorded_instructions, string, unsupported, ALT, SYSTEM,
};

pub use instructions::{RequestedBoundaryEvidence, RequestedResolution};
mod _1_lookup_tables;
use _1_lookup_tables as lookup_tables;

pub use lookup_tables::{assert_requested_parent_alt_still_usable, requested_cannot_invoke_alt};
mod _2_resolution;
use _2_resolution as resolution;

pub use resolution::resolve_requested_dependencies;
mod _3_resolve_requested_dependencies_with_accounts;
use _3_resolve_requested_dependencies_with_accounts as resolve_requested_dependencies_with_accounts;

pub use resolve_requested_dependencies_with_accounts::resolve_requested_dependencies_with_accounts;
mod _4_created_tables;
use _4_created_tables as created_tables;

pub use created_tables::{
    prepare_created_alt_parents, CreatedAltInput, CreatedAltParentProof, CreatedAltParents,
};
