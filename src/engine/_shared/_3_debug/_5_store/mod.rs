//! Create-only, externally pinned local debugger bundles. Integrity is not historical certification.
use super::{error, require};
use crate::shared::{
    diff::canonical_json,
    runtime::{file_sha256, protocol_error, read_bounded_file, ProcessOwner, WorkerSpec},
    trace::{capture_identity, validate_capability, CaptureRequest, ProducerBounds},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use svm_replay_protocol::{
    parse_json, transaction,
    worker::{CaptureWorkerDescriptor, WorkerDescriptor},
    Digest, Error, PreparedRequest,
};
mod _0_types;
use _0_types as types;
pub use types::{
    FileReference, SaveOptions, SavedSession, SessionLineage, SessionManifest, SessionPin,
    SessionSymbols,
};
use types::{ASSURANCE, MAX_BUNDLE, MAX_FILE, MAX_MANIFEST, MAX_SYMBOLS};
mod _1_policy;
use _1_policy as policy;
use policy::{control, encoded, policy, session_identity, validate_request};
mod _2_blobs;
use _2_blobs as blobs;
use blobs::{check_blobs, expected_names};
mod _3_filesystem;
use _3_filesystem as filesystem;
use filesystem::{sync_directory, write_file};
mod _4_bundle;
use _4_bundle as bundle;
use bundle::bundle;
mod _5_save;
use _5_save as save;
pub use save::save_session;
mod _6_open;
use _6_open as open;
use open::load;
pub use open::open_session;
mod _7_branch;
use _7_branch as branch;
pub use branch::branch_session;
mod _8_run_types;
use _8_run_types as run_types;
pub use run_types::{RunArtifacts, RunManifest, SavedRun};
mod _10_run_validation;
use _10_run_validation as run_validation;
use run_validation::{run_blobs, run_limit, run_names};
mod _11_run_save;
use _11_run_save as run_save;
pub use run_save::save_run;
mod _12_run_open;
use _12_run_open as run_open;
pub use run_open::open_run;
