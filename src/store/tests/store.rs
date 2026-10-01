use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;
#[path = "store_cases/fixtures.rs"]
mod fixtures;
use fixtures::{call, get, put};

#[path = "store_cases/restart_roundtrip_and_namespace_isolation.rs"]
mod restart_roundtrip_and_namespace_isolation;

#[path = "store_cases/malformed_requests_and_nonrenewable_leases.rs"]
mod malformed_requests_and_nonrenewable_leases;

#[path = "store_cases/expired_lease_cannot_be_used.rs"]
mod expired_lease_cannot_be_used;
