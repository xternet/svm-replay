use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use svm_replay_engine::shared::sources::{alchemy::*, CompositeSource, HistoricalSource};
use svm_replay_protocol::Digest;
#[path = "alchemy_cases/fixtures.rs"]
mod fixtures;
use fixtures::{account, config, genesis, key, limits, query, Transport};

#[path = "alchemy_cases/public_credential_configuration_is_explicit.rs"]
mod public_credential_configuration_is_explicit;

#[path = "alchemy_cases/exact_account_preserves_u64_and.rs"]
mod exact_account_preserves_u64_and;

#[path = "alchemy_cases/account_data_integer_encoding_and.rs"]
mod account_data_integer_encoding_and;

#[path = "alchemy_cases/duplicate_safe_raw_json_and.rs"]
mod duplicate_safe_raw_json_and;

#[path = "alchemy_cases/original_m1_account_normalization_differential.rs"]
mod original_m1_account_normalization_differential;

#[path = "alchemy_cases/budget_diagnostics.rs"]
mod budget_diagnostics;

#[path = "alchemy_cases/lookup_recovery.rs"]
mod lookup_recovery;

#[path = "alchemy_cases/account_write.rs"]
mod account_write;
