use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use svm_replay_engine::shared::trace::{
    cache::{get_export, put_export, ExportCacheIdentity},
    capture_identity, make_export, CaptureRequest, ProducerBounds,
};
use svm_replay_protocol::{parse_json, worker::WorkerDescriptor, Digest};
use svm_replay_store::Store;
#[path = "trace_cache_cases/fixtures.rs"]
mod fixtures;
use fixtures::{export, identity, raw_get, raw_put};

#[path = "trace_cache_cases/cache_identity_binds_every_input.rs"]
mod cache_identity_binds_every_input;

#[path = "trace_cache_cases/incomplete_publication_and_corruption_are.rs"]
mod incomplete_publication_and_corruption_are;

#[path = "trace_cache_cases/retained_large_export_reopens_byte.rs"]
mod retained_large_export_reopens_byte;
