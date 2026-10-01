//! Provider-attested pre-transaction phase recovery, not reward computation or consensus proof.
use super::*;
use crate::shared::{dependencies::summarize_semantic_transaction, diff::canonical_json};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
mod _0_validation;
use _0_validation as validation;

use validation::{phase_bytes, read_u64, receipt, stake_account, ADJUST_RENT, ERAS, STAKE};
mod _1_rent;
use _1_rent as rent;

use rent::{executed_failure, rent_phase};
mod _2_recovery;
use _2_recovery as recovery;

use recovery::recover;
mod _3_preparation;
use _3_preparation as preparation;

pub use preparation::{prepare_stake_initialization, recover_untouched_reward_stake};

mod _4_nonmutating_accesses;
use _4_nonmutating_accesses as nonmutating_accesses;
use nonmutating_accesses::nonmutating_accesses;

mod _5_inactive_rewards;
use _5_inactive_rewards::initialize_inactive;
pub(crate) use _5_inactive_rewards::{supports_inactive_rewards, EPOCH_REWARDS};
