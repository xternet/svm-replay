//! Offline, pinned directory bundles. No network, shell installer or version fallback.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};
use svm_replay_engine::{
    _1_resolve_runtime::load_catalog,
    shared::runtime::{file_sha256, read_bounded_file},
};
use svm_replay_protocol::{parse_json, worker::WorkerCatalog, Digest, Error};

mod _0_validation;
#[path = "tests.rs"]
#[cfg(test)]
mod tests;

use _0_validation as validation;

#[cfg(test)]
use validation::is_executable;
use validation::{binary_name, error, manifest, regular, relative, set_mode, Entry, Manifest};

pub use validation::Installed;
mod _1_open;
use _1_open as open;

pub use open::{demo_files, open};
mod _2_files;
use _2_files as files;

use files::{copy, new_dir, write_new};
mod _3_pack;
use _3_pack as pack;

pub use pack::{install, pack};
