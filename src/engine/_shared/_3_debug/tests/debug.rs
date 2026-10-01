use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};
use svm_replay_engine::shared::{
    debug::{DebugClient, DebugState, InvocationMetadata, Stop},
    runtime::{CancellationToken, ExecutionBudget},
};
#[path = "debug_cases/fixtures.rs"]
mod fixtures;
use fixtures::{budget, receive, respond};

#[path = "debug_cases/rsp_reads_real_socket_state.rs"]
mod rsp_reads_real_socket_state;

#[path = "debug_cases/delayed_interrupt_does_not_hide.rs"]
mod delayed_interrupt_does_not_hide;
