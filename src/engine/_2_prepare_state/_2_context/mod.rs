//! Preparation compatibility surface; all checks are pure shared Bank services.
pub use crate::shared::bank::snapshots::{
    assert_bound_context, assert_epoch_stake_binding, assert_initialized_stake_binding,
    available_generic_sysvars, parse_sysvar_discovery_response, validate_epoch_stake_failure,
    GENERIC_SYSVARS,
};
