use crate::shared::{
    runtime::ProcessOwner,
    trace::{CaptureRequest, ProducerBounds},
};
use std::path::PathBuf;
use svm_replay_protocol::Digest;
#[derive(Clone)]
pub struct Config {
    pub catalog_path: PathBuf,
    pub catalog_sha256: Digest,
    pub data_dir: PathBuf,
    pub owner: ProcessOwner,
    pub cache: CacheMode,
    pub trace: Option<TraceOptions>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceOptions {
    pub capture: CaptureRequest,
    pub bounds: ProducerBounds,
    /// Reject incomplete exports instead of accepting a bounded partial capture.
    #[serde(default)]
    pub require_complete: bool,
}
mod _0_completeness;
#[cfg(test)]
mod tests;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheMode {
    Off,
    Prepared,
    All,
}
