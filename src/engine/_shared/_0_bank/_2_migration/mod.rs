//! Finite provider-attested program migration images, not a historical RPC replacement.
use super::*;
use crate::shared::{
    dependencies::summarize_semantic_transaction,
    diff::canonical_json,
    history::{self, Roles},
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
mod _0_validation;
use _0_validation as validation;

use validation::{
    account, data, error, get, has_feature, receipt, strict, Migration, ALPENGLOW, LOADER,
    MIGRATIONS, RELAX, RENT, SYSTEM,
};
mod _1_proof;
use _1_proof as proof;

use proof::prove;
mod _2_overlays;
use _2_overlays as overlays;

pub use overlays::{
    program_migration_context, resolve_program_migration_overlays,
    validate_program_migration_parents,
};
mod _3_binding;
use _3_binding as binding;

pub use binding::assert_program_migration_bindings;
