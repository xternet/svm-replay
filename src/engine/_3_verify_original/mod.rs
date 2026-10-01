//! Archived evidence verification; execution and original-control ordering live upstream.
mod _0_metadata;
use _0_metadata as metadata;
mod _1_trace;
use _1_trace as trace;

pub use metadata::{compare_archived_compute_metadata, MetadataDivergence, MetadataPolicy};
pub use trace::{assert_runtime_instruction_trace, verify_runtime_instruction_trace};

use crate::shared::diff::{canonical_json, exact_u64};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
mod _2_validation;
use _2_validation as validation;

pub use validation::{VerificationError, VerificationReport};

use validation::{
    array, base64_bytes, check, equal, field, hash, integer, object, string, strings, unsigned,
    usize_value, Result,
};
mod _3_archive;
use _3_archive as archive;

use archive::{archived_transaction, ArchivedTransaction};
mod _4_state;
use _4_state as state;

use state::{archived_state, nullable_unsigned, return_data, state_lamports, token_amounts};
mod _5_execution;
use _5_execution as execution;

use execution::verify_execution;
mod _6_requested;
use _6_requested as requested;

pub use requested::assert_requested_execution_shape;

use requested::{execution_behavior, nullable_object, requested_override_proof, require_trace};
mod _7_verification;
use _7_verification as verification;

pub use verification::verify;
mod _8_verify_with_options;
use _8_verify_with_options as verify_with_options;

pub use verify_with_options::verify_with_options;
