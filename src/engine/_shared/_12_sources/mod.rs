//! Exact historical observations. Integrity is not proof of consensus or completeness.
pub mod _0_alchemy;
pub use _0_alchemy as alchemy;
pub mod _1_cached;
pub use _1_cached as cached;
mod _2_supplied;
use _2_supplied as supplied;
pub use supplied::SuppliedBankSource;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use svm_replay_protocol::{parse_json, validate_safe_numbers, Digest};
mod _3_contract;
use _3_contract as contract;

pub use contract::{HistoricalSource, Result, SourceError};

use contract::{
    address, hash, json_value, local_diagnostics, object, public_id, require, slot, text,
};
mod _4_query;
use _4_query as query;

use query::value_hash;

pub use query::query_key;
mod _5_identity;
use _5_identity as identity;

use identity::{covers, validate_identity};
mod _6_records;
use _6_records as records;

use records::{
    canonical_base64, evidence_hashes, parse_source_json, unique_hashes, validate_record,
};
mod _7_captured;
use _7_captured as captured;

pub use captured::CapturedSource;
mod _8_composite;
use _8_composite as composite;

pub use composite::CompositeSource;
