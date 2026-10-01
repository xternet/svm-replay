//! Exact EpochRewards evidence can prove no pre-transaction stake distribution.
use super::*;
mod _0_implementation;
pub(super) use _0_implementation::initialize_inactive;
pub(crate) use _0_implementation::{supports_inactive_rewards, EPOCH_REWARDS};
