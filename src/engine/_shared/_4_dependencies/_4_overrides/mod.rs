//! Requested-only shape and inspection checks. Never installs changes in historical state.
use super::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use svm_replay_protocol::Error;
mod _0_validation;
use _0_validation as validation;

use validation::LOADER;

pub use validation::{
    overridden_lookup_needs_slot_hashes, validate_requested_overrides, RequestedAccountOverride,
    OVERRIDABLE_SYSVARS,
};

pub(super) use validation::{bytes, checked_overrides, invalid, u32_at, u64_at};
mod _1_assert_override_membership;
use _1_assert_override_membership as assert_override_membership;

pub use assert_override_membership::{assert_override_membership, requested_image_inspection};
