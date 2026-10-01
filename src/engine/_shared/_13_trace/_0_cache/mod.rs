//! Immutable observation reuse, not execution or historical certification.
//! The caller supplies its validated context pin; this module never skips a live gate.
use super::{capture_identity, validate_artifact, CaptureExport, CaptureRequest, ProducerBounds};
use crate::shared::diff::canonical_json;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use svm_replay_protocol::{parse_json, worker::WorkerDescriptor, Digest, Error};
use svm_replay_store::Store;
mod _0_storage;
use _0_storage as storage;

use storage::{
    corruption, get, payload_key, put, receipt, validate_export, Manifest, PayloadRef,
    MAX_MANIFEST_BYTES, MAX_PAYLOAD_BYTES,
};

pub use storage::{CachedExport, ExportCacheIdentity, ExportCacheReceipt};
mod _1_get_export;
use _1_get_export as get_export;

pub use get_export::{get_export, put_export};
