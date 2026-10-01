use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use svm_replay_engine::shared::sources::{
    query_key, CapturedSource, CompositeSource, HistoricalSource, SourceError, SuppliedBankSource,
};
use svm_replay_protocol::Digest;
#[path = "sources_cases/fixtures.rs"]
mod fixtures;
use fixtures::{account, bank, controlled, hash, identity, key, Capture};

#[path = "sources_cases/source_network_is_bound_even.rs"]
mod source_network_is_bound_even;

#[path = "sources_cases/returned_query_and_account_boundary.rs"]
mod returned_query_and_account_boundary;

#[path = "sources_cases/transaction_wire_must_bind_requested.rs"]
mod transaction_wire_must_bind_requested;

#[path = "sources_cases/bad_query_phase_network_numbers.rs"]
mod bad_query_phase_network_numbers;

#[path = "sources_cases/block_discovery.rs"]
mod block_discovery;
