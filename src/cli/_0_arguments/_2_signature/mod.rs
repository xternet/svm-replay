//! Thin signature front door; historical logic stays in the numbered engine.
mod _0_controls;
use _0_controls as controls;
mod _1_credentials;
use _1_credentials as credentials;
mod _2_inputs;
use _2_inputs as inputs;
pub(super) mod _3_installation;
pub(super) use _3_installation as installation;
mod _4_output;
use _4_output as output;
mod _5_provider;
use _5_provider as provider;
mod _6_wizard;
use _6_wizard as wizard;
use clap::Args;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc, time::Instant};
use svm_replay_engine::{
    _1_resolve_runtime::load_catalog,
    _2_prepare_state::source::signature::prepare_with_registry,
    shared::{
        history::source_error,
        runtime::ProcessOwner,
        sources::{CapturedSource, HistoricalSource},
    },
    simulate_historical, CacheMode, Config,
};
use svm_replay_protocol::{Digest, Error};
pub(super) use wizard::collect;
mod _7_execution;
use _7_execution as execution;

pub(super) use execution::run;
mod _8_options;
pub(super) use _8_options::Options;
mod _9_historical;
use _9_historical as historical;
pub(crate) use controls::Controls as SignatureControls;
pub(crate) use historical::Options as HistoricalOptions;
mod _10_session;
pub(crate) use _10_session::{prepare_session, SessionPreparation};
