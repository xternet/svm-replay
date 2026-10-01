use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use svm_replay_engine::shared::{
    dependencies::{
        overrides::{
            assert_override_membership, overridden_lookup_needs_slot_hashes,
            requested_image_inspection, validate_requested_overrides, RequestedAccountOverride,
            OVERRIDABLE_SYSVARS,
        },
        requested::{
            assert_requested_parent_alt_still_usable, prepare_created_alt_parents,
            requested_cannot_invoke_alt, resolve_requested_dependencies_with_accounts,
            CreatedAltInput, RequestedBoundaryEvidence,
        },
        summarize_semantic_transaction, SemanticTransaction,
    },
    history::Roles,
};
#[path = "requested_cases/fixtures.rs"]
mod fixtures;
use fixtures::{
    absent, account, boundary, edit, key, mutation, original, programs, resolve, roles, summaries,
    table, wire, ALT,
};

#[path = "requested_cases/override_shape_preserves_u64_strings.rs"]
mod override_shape_preserves_u64_strings;

#[path = "requested_cases/same_slot_extend_freeze_and.rs"]
mod same_slot_extend_freeze_and;

#[path = "requested_cases/override_membership_accepts_related_backing.rs"]
mod override_membership_accepts_related_backing;

#[path = "requested_cases/created_parent_without_data_edit.rs"]
mod created_parent_without_data_edit;
