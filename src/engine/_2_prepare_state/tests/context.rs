use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use svm_replay_engine::_2_prepare_state::context::{
    assert_bound_context, assert_epoch_stake_binding, assert_initialized_stake_binding,
    available_generic_sysvars, parse_sysvar_discovery_response, validate_epoch_stake_failure,
};
use svm_replay_engine::shared::bank::snapshots::{
    bind_epoch_stake_evidence, prepare_exact_initialized_stakes,
};
use svm_replay_engine::shared::diff::canonical_json;
#[path = "context_cases/fixtures.rs"]
mod fixtures;
use fixtures::{discovery_fixture, evidence, fixture, hash, initialized_request, key, reseal};

#[path = "context_cases/exact_bound_context_accepts_and.rs"]
mod exact_bound_context_accepts_and;

#[path = "context_cases/source_binding_ports_snapshot_field.rs"]
mod source_binding_ports_snapshot_field;

#[path = "context_cases/epoch_snapshot_rejects_malformed_schema.rs"]
mod epoch_snapshot_rejects_malformed_schema;

#[path = "context_cases/zero_empty_and_u64_max.rs"]
mod zero_empty_and_u64_max;

#[path = "context_cases/generic_availability_requires_target_bytes.rs"]
mod generic_availability_requires_target_bytes;
