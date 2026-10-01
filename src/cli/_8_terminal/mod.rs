mod _0_style;
use _0_style as style;
mod _1_trace;
use _1_trace as trace;
use dialoguer::Select;
use serde_json::Value;
use std::{
    io::{IsTerminal, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use svm_replay_protocol::Error;

mod _2_presentation;
use _2_presentation as presentation;
#[path = "tests.rs"]
#[cfg(test)]
mod tests;

pub use presentation::{clean, configure, display, enabled, interactive, section, select, stage};
mod _3_progress;
use _3_progress as progress;

pub use progress::Progress;

#[cfg(test)]
use progress::{animate_progress, progress_end};
mod _4_receipt;
use _4_receipt as receipt;

pub use receipt::render;

#[cfg(test)]
use receipt::{format_result, result_summary};
