//! Integrity-only local storage. Historical provenance and completeness belong to the caller.
use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod _0_types;
#[path = "tests/crash_tests.rs"]
#[cfg(test)]
mod crash_tests;

use _0_types as types;

pub use types::{Result, StoreError, MAX_BLOB_BYTES, MAX_REQUEST_BYTES};

use types::{Command, Counts, Lease, MAX_INTEGER};
mod _1_validation;
use _1_validation as validation;

use validation::{integer, now_ms, sha256, text_id, version};
mod _2_database;
use _2_database as database;

pub use database::Store;
mod _3_execute;
mod _4_leases;
use _4_leases as leases;

use leases::{
    acquire_lease, entry, lease_value, next_seq, require_entry, validate_lease, validate_owned,
};
mod _5_blobs;
use _5_blobs as blobs;

use blobs::{collect_garbage, evict, publish_blob, read_blob};
mod _6_budget;
use _6_budget as budget;

use budget::budget;
mod _7_request;
use _7_request as request;

#[cfg(test)]
use request::crash_point;
pub use request::parse_request;
