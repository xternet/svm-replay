//! Bounded native CLI output. Synchronous Windows I/O runs on an owned thread
//! so the coordinator can cancel a blocked pipe without detaching that thread.
use serde_json::Value;
use std::{
    io::{self, Write},
    os::windows::io::AsRawHandle,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use svm_replay_protocol::Error;
use windows_sys::Win32::{Foundation::ERROR_NOT_FOUND, System::IO::CancelSynchronousIo};

mod _0_output;
use _0_output as output;
pub(crate) use output::write_final;

#[path = "tests.rs"]
#[cfg(test)]
mod tests;

mod _1_execution;
pub use _1_execution::run;
