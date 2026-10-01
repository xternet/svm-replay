use super::{error, require};
use crate::shared::runtime::{protocol_error, ExecutionBudget};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::{self, Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream},
    thread,
    time::Duration,
};
use svm_replay_protocol::Error;
mod _0_client;
use _0_client as client;

pub use client::{DebugClient, DebugState, Stop};

use client::{hex, io_error, unhex, PendingKind, Reply};
mod _1_breakpoint;
mod _2_packet;
mod _3_poll_inner;
