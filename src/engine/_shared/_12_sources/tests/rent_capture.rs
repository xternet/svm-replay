//! Deliberate, bounded one-shot corpus acquisition; never part of normal tests.
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use svm_replay_engine::shared::sources::{
    alchemy::*, CapturedSource, HistoricalSource, SourceError,
};
use svm_replay_protocol::Digest;
#[path = "rent_capture_cases/fixtures.rs"]
mod fixtures;
use fixtures::{
    checked_query, config, create_private, freeze, invalid, io, limits, require, validate_frozen,
    validate_queries, write_new, Result, GENESIS, RENT,
};

#[path = "rent_capture_cases/capture.rs"]
mod capture;
use capture::{
    capture, env, new_output, query, read_pinned, synthetic_summary, SyntheticTransport,
};

#[path = "rent_capture_cases/freeze_reviewed_rent_queries.rs"]
mod freeze_reviewed_rent_queries;
