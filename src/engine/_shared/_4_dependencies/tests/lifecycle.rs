use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use svm_replay_engine::shared::{
    dependencies::{lifecycle::*, summarize_semantic_transaction},
    history::Roles,
};
#[path = "lifecycle_cases/fixtures.rs"]
mod fixtures;
use fixtures::{absent, account, context, inspect, key, raw, V4};

#[path = "lifecycle_cases/existing_v3_finite_instruction_forms.rs"]
mod existing_v3_finite_instruction_forms;

#[path = "lifecycle_cases/deploy_parent_exception_does_not.rs"]
mod deploy_parent_exception_does_not;
