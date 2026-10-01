//! Experimental native CLI JSONL bridge; not an end-user interactive debugger.
//! One scoped engine thread; no background stdin reader.
//! Commands are strict DebugCommand JSON, one LF-terminated line, <=64 KiB each.
//! EOF/signals/errors cancel and join the engine before returning its receipt.
use serde_json::Value;
use svm_replay_engine::{shared::runtime::CancellationToken, Config};
use svm_replay_protocol::{Error, PreparedRequest};

#[cfg(windows)]
mod _0_windows;
#[cfg(windows)]
use _0_windows as windows;
#[cfg(any(unix, windows))]
use svm_replay_engine::shared::debug::{DebugCommand, DebugController};
#[cfg(windows)]
pub use windows::run;
#[cfg(windows)]
pub(crate) use windows::write_final;

#[cfg(unix)]
mod _1_unix;
#[cfg(unix)]
use _1_unix as unix;
#[cfg(unix)]
pub use unix::run;
#[cfg(unix)]
pub(crate) use unix::write_final;
mod _2_input;
use _2_input as input;
#[cfg(not(any(unix, windows)))]
pub use input::run;
#[cfg(not(any(unix, windows)))]
pub(crate) use input::write_final;
#[cfg(test)]
use input::MAX_LINE;
#[cfg(any(unix, windows))]
use input::{Input, MAX_STREAM};
