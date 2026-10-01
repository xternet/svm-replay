use serde_json::{json, Value};
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::Path, process::Command};
use svm_replay_protocol::Digest;
#[path = "product_cases/fixtures.rs"]
mod fixtures;
#[cfg(target_os = "linux")]
use fixtures::catalog;
use fixtures::{cli, response_pin};

#[path = "product_cases/documented_trace_configuration_matches_public.rs"]
mod documented_trace_configuration_matches_public;

#[path = "product_cases/bundle_rejects_bad_paths_duplicates.rs"]
mod bundle_rejects_bad_paths_duplicates;
