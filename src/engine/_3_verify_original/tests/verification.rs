use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use svm_replay_engine::_3_verify_original::{
    compare_archived_compute_metadata, verify, verify_runtime_instruction_trace,
    verify_with_options, MetadataPolicy,
};
#[path = "verification_cases/fixtures.rs"]
mod fixtures;
use fixtures::{context, hash, trace_context};

#[path = "verification_cases/exact_integer_evidence_verifies_without.rs"]
mod exact_integer_evidence_verifies_without;

#[path = "verification_cases/warning_policy_only_accepts_explained.rs"]
mod warning_policy_only_accepts_explained;

#[path = "verification_cases/normalized_error_wire_never_aliases.rs"]
mod normalized_error_wire_never_aliases;

#[path = "verification_cases/preserved_m6_archive_verifies_and.rs"]
mod preserved_m6_archive_verifies_and;

#[path = "verification_cases/fee_rewards.rs"]
mod fee_rewards;

#[path = "verification_cases/nested_meter.rs"]
mod nested_meter;

#[path = "verification_cases/token_metadata.rs"]
mod token_metadata;

#[path = "verification_cases/load_failure.rs"]
mod load_failure;
