use super::sysvars::SYSVAR_OWNER;
use super::*;
use serde_json::json;
mod _0_binding;
use _0_binding as binding;

pub use binding::{
    assert_last_restart_slot_binding, last_restart_account_hash, last_restart_binding_hash,
    prepare_last_restart_slot, LAST_RESTART_SLOT, LAST_RESTART_SOURCE,
};
mod _1_include_last_restart_slot_account;
use _1_include_last_restart_slot_account as include_last_restart_slot_account;

pub use include_last_restart_slot_account::{
    include_last_restart_slot_account, prepare_last_restart_from_observation,
};
