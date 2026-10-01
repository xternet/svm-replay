use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use svm_replay_engine::shared::bank::stake::{
    prepare_stake_initialization, recover_untouched_reward_stake,
};
#[path = "stake_cases/fixtures.rs"]
mod fixtures;
use fixtures::{
    adjusted, bytes, change, change_bytes, hash, input, instruction, key, payload, prepared, strict,
};

#[path = "stake_cases/all_reviewed_eras_and_nonmutating.rs"]
mod all_reviewed_eras_and_nonmutating;

#[path = "stake_cases/zero_reward_credits_can_stay.rs"]
mod zero_reward_credits_can_stay;

#[path = "stake_cases/legacy_rent_markers_and_deactivated.rs"]
mod legacy_rent_markers_and_deactivated;

#[path = "stake_cases/inactive_rewards.rs"]
mod inactive_rewards;
