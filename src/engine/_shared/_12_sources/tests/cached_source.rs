use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use svm_replay_engine::shared::sources::{cached::*, HistoricalSource, SourceError};
use svm_replay_protocol::Digest;
use svm_replay_store::Store;
#[path = "cached_source_cases/fixtures.rs"]
mod fixtures;
use fixtures::{identity, key, query, record, scope, Source};

#[path = "cached_source_cases/repeated_variant_reuses_exact_absence.rs"]
mod repeated_variant_reuses_exact_absence;

#[path = "cached_source_cases/contradictory_concurrent_fills_on_independent.rs"]
mod contradictory_concurrent_fills_on_independent;
