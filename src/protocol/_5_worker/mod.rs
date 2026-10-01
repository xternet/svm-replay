use crate::Digest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerDescriptor {
    pub family: String,
    pub file: String,
    pub sha256: Digest,
    pub build_hash: Digest,
    pub source_sha256: Digest,
    pub executor_source_id: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Platform {
    pub os: String,
    pub arch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glibc_min: Option<String>,
}

/// Manifest bytes require an independently trusted digest supplied by the caller.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerCatalog {
    pub schema: String,
    pub platform: Platform,
    pub workers: Vec<WorkerDescriptor>,
    /// Separate reviewed observation binaries; omission never selects a fallback.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capture_workers: Vec<CaptureWorkerDescriptor>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureWorkerDescriptor {
    pub worker: WorkerDescriptor,
    pub reference_sha256: Digest,
    /// Externally reviewed passing gate receipt, attested by the catalog pin.
    pub gate_sha256: Digest,
}
