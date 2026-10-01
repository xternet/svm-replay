//! Explicit override baselines retain their reviewed historical Bank phase.
use super::{lifecycle::unsupported, overrides::RequestedAccountOverride, unique, Result};
use crate::shared::{
    bank::sysvars::{bind_exact_generic_sysvar, BANK_INITIALIZED_SYSVARS, SYSVAR_OWNER},
    diff::canonical_json,
    history::{account_data, classify_account, History, Roles, CLOCK, INSTRUCTIONS},
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use svm_replay_protocol::{Digest, Error};
mod _0_hydration;
use _0_hydration as hydration;

use hydration::{
    assert_image, bank_source, features, valid_boundary, PARENT_SYSVARS, POLICY, SLOT_HASHES,
};

pub use hydration::{
    hydrate_override_sysvar_baselines, OverrideSysvarBaselines, OverrideSysvarContext,
};
mod _1_hydrate_override_sysvar_baselines_with_accounts;
use _1_hydrate_override_sysvar_baselines_with_accounts as hydrate_override_sysvar_baselines_with_accounts;

pub use hydrate_override_sysvar_baselines_with_accounts::{
    hydrate_override_sysvar_baselines_with_accounts, proven_parent_override_sysvars,
};
