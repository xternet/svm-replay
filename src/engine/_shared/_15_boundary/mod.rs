//! Historical fork projections from independently pinned replay jobs; no execution or provider access.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use svm_replay_protocol::{parse_json, Digest, Error};
mod _0_images;
mod _1_jobs;
mod _2_history;
mod _3_features;
mod _4_expansion;
mod _5_project;
use _0_images::{array, bytes, field, image, key, number, object, string, Images};
use _1_jobs::Job;
pub use _5_project::{project, project_file};
const SYSVAR: &str = "Sysvar1111111111111111111111111111111111111";
const RENT: &str = "SysvarRent111111111111111111111111111111111";
const FEATURE: &str = "Feature111111111111111111111111111111111111";
const UPGRADEABLE: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const SLOT_SYSVARS: [&str; 2] = [
    "SysvarS1otHashes111111111111111111111111111",
    "SysvarRecentB1ockHashes11111111111111111111",
];
fn invalid(message: impl Into<String>) -> Error {
    Error::new("BOUNDARY_FORMAT", message)
}
#[cfg(test)]
mod tests;
