//! Pure captured-metadata summaries and causal prefix selection, not runtime admission.
pub mod _0_alt;
pub use _0_alt as alt;
mod _1_closure;
use _1_closure as closure;
pub mod _2_lifecycle;
pub use _2_lifecycle as lifecycle;
pub mod _3_override_sysvars;
pub use _3_override_sysvars as override_sysvars;
pub mod _4_overrides;
pub use _4_overrides as overrides;
pub mod _5_requested;
pub use _5_requested as requested;
mod _6_transaction;
use _6_transaction as transaction;
pub use closure::{compute_backward_semantic_closure, compute_program_data_fixed_point};
pub use transaction::{include_requested_dependencies, summarize_semantic_transaction};
pub mod _8_credits;

use serde::Serialize;
use std::collections::BTreeSet;
use svm_replay_protocol::Error;
mod _7_validation;
use _7_validation as validation;

pub use validation::{
    Result, SemanticClosureResult, SemanticTransaction, WritableIntersection, VOTE_PROGRAM,
};

use validation::{invalid, pubkey, safe_integer, unique, unique_sorted, EXCLUDED};
