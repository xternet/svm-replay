//! Immutable raw observation cache. No transient/unavailable result is cached.
use super::*;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use svm_replay_store::Store;
mod _0_source;
use _0_source as source;

pub use source::{raw_cache_key, source_identity_sha256, CacheScope, CachedSource};

use source::execute;
mod _1_source_validation;
