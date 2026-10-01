#[path = "tests.rs"]
#[cfg(test)]
mod tests;
use serde_json::Value;
use std::{fs::OpenOptions, io::Write, path::Path};
mod _0_issue;
use _0_issue as issue;

pub use issue::{write_if_internal, ISSUE_URL};
