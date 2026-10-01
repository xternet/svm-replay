//! macOS owner: monitor the CLI parent, terminate the worker group, reap the
//! direct child. Darwin reaps orphaned descendants; receipts state group scope.
use std::{
    ffi::OsString,
    io,
    os::unix::process::{CommandExt, ExitStatusExt},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

mod _0_implementation;

use _0_implementation::{terminate, STOP};

pub use _0_implementation::run;
