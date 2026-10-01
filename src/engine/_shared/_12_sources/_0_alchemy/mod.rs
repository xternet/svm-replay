//! Bounded exact historical RPC adapter. Construction never makes network calls.
use super::*;
use std::{
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
mod _0_budget;
use _0_budget as budget;
mod _1_discovery;
mod _2_discovery_cache;
pub use budget::DurableRpcBudget;

mod _3_transport;
use _3_transport as transport;
#[path = "tests.rs"]
#[cfg(test)]
mod transport_tests;

pub use transport::{
    AlchemyConfig, RpcHttpResponse, RpcTransport, SourceCounters, SourceLimits, TransportFailure,
};

use transport::HttpsTransport;
mod _4_client;
use _4_client as client;

use client::State;

pub use client::AlchemySource;
mod _5_post;
mod _6_response;
use _6_response as response;

use response::{checked_genesis, decimal, envelope, parse, record_from_response, validate_limits};

pub(super) use response::request_for_query;
mod _7_values;
use _7_values as values;

pub(super) use values::value_from_response;
mod _8_provenance;
use _8_provenance as provenance;

pub(super) use provenance::validate_provenance;
mod _10_source;
mod _11_lookup;
