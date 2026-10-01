use super::{
    ExecutionBudget, ProcessOwner, WorkerError, WorkerErrorCode, WorkerLimits, WorkerSpec,
};
use std::path::{Path, PathBuf};

pub(super) struct Execution {
    pub pid: u32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[cfg(windows)]
mod _0_windows;
#[cfg(windows)]
use _0_windows as windows;
#[cfg(windows)]
pub(super) use windows::execute;

#[cfg(not(any(unix, windows)))]
pub(super) fn execute(
    _: &WorkerSpec,
    _: Option<&ProcessOwner>,
    _: &Path,
    _: &Path,
    _: &Path,
    _: &[String],
    _: &[PathBuf],
    _: &WorkerLimits,
    _: &ExecutionBudget,
    _: Option<&std::sync::mpsc::SyncSender<Vec<u8>>>,
) -> Result<Execution, WorkerError> {
    Err(WorkerError::new(
        WorkerErrorCode::UnsupportedPlatform,
        "native worker transport is qualified only on Linux",
    ))
}

#[cfg(unix)]
mod _1_unix;
#[cfg(unix)]
use _1_unix as unix;
#[cfg(unix)]
pub(super) use unix::execute;
