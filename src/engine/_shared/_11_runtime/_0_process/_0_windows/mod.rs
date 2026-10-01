//! Windows workers belong to a kill-on-close Job Object from suspended launch.
//! A private scoped runtime keeps the synchronous API safe inside async hosts.
use super::{
    Execution, ExecutionBudget, ProcessOwner, WorkerError, WorkerErrorCode, WorkerLimits,
    WorkerSpec,
};
use process_wrap::tokio::{CommandWrap, JobObject, KillOnDrop};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::mpsc::SyncSender,
    time::Duration,
};
use tokio::io::AsyncReadExt;

mod _0_implementation;

pub(in super::super) use _0_implementation::execute;

use _0_implementation::{append, send};
