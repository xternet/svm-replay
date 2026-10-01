//! Resolve an explicitly pinned installed runtime; never substitute another family.
pub mod _2_registry;
use crate::shared::runtime::{file_sha256, read_bounded_file, WorkerSpec};
pub use _2_registry as registry;
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};
use svm_replay_protocol::{
    parse_json,
    worker::{WorkerCatalog, WorkerDescriptor},
    Digest, Error, PreparedRequest,
};
mod _0_catalog;
use _0_catalog as catalog;

pub use catalog::{load_catalog, InstalledCatalog, ResolvedCaptureWorker, ResolvedWorker};

use catalog::{confined, problem};
mod _1_resolve;
use _1_resolve as resolve;

pub use resolve::{resolve, resolve_binding, resolve_capture, validate_worker_binding};
