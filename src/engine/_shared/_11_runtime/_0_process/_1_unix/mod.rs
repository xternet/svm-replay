mod _0_cleanup;
use _0_cleanup as cleanup;
use cleanup::cleanup;
mod _1_diagnostics;
use _1_diagnostics as diagnostics;
use diagnostics::collect;
mod _2_spawn;
use super::*;
use _2_spawn as spawn;
use spawn::spawn;
use std::{
    io::{self, Read},
    os::{
        fd::AsRawFd,
        unix::process::{CommandExt, ExitStatusExt},
    },
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

mod _3_execution;
pub(crate) use _3_execution::execute;

use _3_execution::{group_exists, signal};
