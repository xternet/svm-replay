//! Signature discovery feeds the existing historical reconstruction contract.
use crate::{
    _1_resolve_runtime::{registry::Registry, InstalledCatalog},
    _2_prepare_state::boundary::boundary,
    shared::{history::source_error, sources::alchemy::AlchemySource},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value;
use svm_replay_protocol::{Error, HistoricalRequest, Limits, MetadataPolicy};

mod _0_implementation;

pub use _0_implementation::{
    prepare, prepare_after_slot_with_registry, prepare_initialization_with_registry,
    prepare_with_registry,
};
