//! Exact ordinary Program headers from bound native lifecycle evidence.
use super::*;
use crate::shared::diff::canonical_json;
use serde_json::json;
use std::collections::BTreeSet;
mod _0_derive;
mod _1_witness;
mod _2_interval;
mod _3_fields;
pub use _0_derive::derive;
use _1_witness::{first_balance, upgrade};
use _2_interval::review;
use _3_fields::{account_bytes, rent_minimum};
const LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const REVIEWED: [&str; 2] = ["litesvm-v0.6.1-agave-2.2.20", "litesvm-v0.7.1-agave-2.3.9"];
const BANK_MANAGED: [&str; 5] = [
    "Stake11111111111111111111111111111111111111",
    "Config1111111111111111111111111111111111111",
    "AddressLookupTab1e1111111111111111111111111",
    "Feature111111111111111111111111111111111111",
    "S1asHs4je6wPb2kWiHqNNdpNRiDaBEDQyfyCThhsrgv",
];
