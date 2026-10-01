use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use svm_replay_engine::shared::{
    bank::migration::{
        assert_program_migration_bindings, program_migration_context,
        resolve_program_migration_overlays, validate_program_migration_parents,
    },
    diff::canonical_json,
};
#[path = "migration_cases/fixtures.rs"]
mod fixtures;
use fixtures::{body, change, change_bytes, fixture, hash, image, input, key, ROWS};

#[path = "migration_cases/every_reviewed_migration_derives_exact.rs"]
mod every_reviewed_migration_derives_exact;

#[path = "migration_cases/prefunded_token_data_requires_relax.rs"]
mod prefunded_token_data_requires_relax;
