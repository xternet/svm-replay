use super::*;
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Write},
    os::fd::AsRawFd,
    os::unix::net::UnixStream,
};
#[path = "tests_cases/fixtures.rs"]
mod fixtures;
use fixtures::{command, fill};

#[path = "tests_cases/final_json_can_exceed_debug.rs"]
mod final_json_can_exceed_debug;
