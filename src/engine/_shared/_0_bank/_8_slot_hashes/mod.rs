//! Exact data-only SlotHashes from finalized successful native vote evidence.
//! This does not prove sysvar account metadata or independently prove consensus.
use super::*;
use crate::shared::diff::canonical_json;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
mod _0_votes;
mod _1_chain;
pub use _1_chain::{reconstruct, Reconstruction};
mod _2_binding;
pub use _2_binding::{bind_data, bind_fixture};

pub const SLOT_HASHES: &str = "SysvarS1otHashes111111111111111111111111111";
pub const ENTRIES: usize = 512;

fn reviewed(executor: &str) -> Result<()> {
    check(
        matches!(
            executor,
            "litesvm-v0.6.1-agave-2.2.20"
                | "litesvm-v0.8.2-agave-3.0.10"
                | "litesvm-v0.12.0-agave-3.1.11"
                | "litesvm-v0.14.0-pr402-agave-4.1.2"
        ),
        "unreviewed vote-hash runtime",
    )
}
