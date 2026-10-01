//! Pure historical Bank derivation and exact caller-supplied sysvar evidence.
pub mod _0_context;
pub use _0_context as context;
pub mod _1_imports;
pub use _1_imports as imports;
pub mod _2_migration;
pub use _2_migration as migration;
pub mod _3_restart;
pub use _3_restart as restart;
pub mod _4_snapshots;
pub use _4_snapshots as snapshots;
pub mod _5_stake;
pub use _5_stake as stake;
pub mod _6_sysvars;
pub use _6_sysvars as sysvars;

use crate::shared::diff::exact_u64;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use svm_replay_protocol::Error;
mod _7_validation;
use _7_validation as validation;

use validation::{
    array, bytes, check, digest, fail, field, hash, integer, object, pubkey, string, uint, Result,
};
pub mod _8_slot_hashes;
pub use _8_slot_hashes as slot_hashes;
pub mod _9_program_header;
pub use _9_program_header as program_header;
