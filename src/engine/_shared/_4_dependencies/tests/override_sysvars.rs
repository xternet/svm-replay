use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use svm_replay_engine::shared::{
    bank::sysvars::{bind_exact_generic_sysvar, BANK_INITIALIZED_SYSVARS},
    dependencies::{override_sysvars::*, overrides::RequestedAccountOverride},
};
#[path = "override_sysvars_cases/fixtures.rs"]
mod fixtures;
use fixtures::{account, context, edits, EXECUTORS, LEGACY, SLOT_HASHES};

#[path = "override_sysvars_cases/all_six_sources_bind_ten.rs"]
mod all_six_sources_bind_ten;

#[path = "override_sysvars_cases/history_adapter_preserves_exact_genesis.rs"]
mod history_adapter_preserves_exact_genesis;

#[path = "override_sysvars_cases/original_override_sysvar_differential_binds.rs"]
mod original_override_sysvar_differential_binds;
