use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use svm_replay_engine::shared::dependencies::{
    compute_backward_semantic_closure, compute_program_data_fixed_point,
    include_requested_dependencies, summarize_semantic_transaction, SemanticTransaction,
};
#[path = "dependencies_cases/fixtures.rs"]
mod fixtures;
use fixtures::{nonce, raw, reference_paths, semantic, simple, summary, RECENT, SYSTEM};

#[path = "dependencies_cases/fee_only_shared_payer_does.rs"]
mod fee_only_shared_payer_does;

#[path = "dependencies_cases/malformed_metadata_headers_and_instruction.rs"]
mod malformed_metadata_headers_and_instruction;

#[path = "dependencies_cases/backward_scan_matches_exhaustive_causal.rs"]
mod backward_scan_matches_exhaustive_causal;

#[path = "dependencies_cases/vote_authority.rs"]
mod vote_authority;
