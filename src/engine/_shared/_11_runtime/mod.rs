//! Hash-pinned native worker transport. This is process isolation, not a sandbox.
mod _0_process;
use _0_process as process;

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
mod _1_budget;
use _1_budget as budget;

pub use budget::{CancellationToken, ExecutionBudget};
mod _2_types;
use _2_types as types;

pub use types::{
    protocol_error, CleanupScope, ProcessOwner, WorkerError, WorkerErrorCode, WorkerLimits,
    WorkerOutput, WorkerSpec, WorkerTransport,
};
mod _3_files;
use _3_files as files;
mod _4_run_observed;
mod _5_transport;

use files::verify_pin;

pub use files::{file_sha256, read_bounded_file};
mod _6_streams;
use _6_streams as streams;

use streams::write_input;

pub(crate) use streams::check_output;
