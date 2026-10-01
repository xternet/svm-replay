//! CLI source construction; provider-independent execution lives in the engine.
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use svm_replay_engine::{
    shared::{
        history::source_error,
        runtime::read_bounded_file,
        sources::{
            alchemy::{AlchemyConfig, AlchemySource, DurableRpcBudget, SourceLimits},
            cached::{source_identity_sha256, CacheScope, CachedSource},
            CapturedSource, CompositeSource, HistoricalSource,
        },
    },
    CacheMode,
};
use svm_replay_protocol::{parse_json, Digest, Error};

mod _0_configuration;
use _0_configuration as configuration;
#[path = "tests.rs"]
#[cfg(test)]
mod tests;

#[cfg(test)]
use configuration::LiveConfig;

pub use configuration::{alchemy_source, configured};

pub(crate) use configuration::finish_sources;
