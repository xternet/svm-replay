//! Live interpreter execution gated by fresh, same-input JIT replay/capture parity.
use super::trace::{self, TraceContext};
use crate::{
    shared::{
        debug::{run_session, validate_finalized_inventory, DebugDriver, FinalizedInventoryPolicy},
        diff::canonical_json,
        runtime::{file_sha256, protocol_error, read_bounded_file, ExecutionBudget, WorkerLimits},
        trace::{
            capture_identity, make_export, validate_capability, CaptureExport, CaptureRequest,
            ProducerBounds,
        },
    },
    {
        _1_resolve_runtime::{ResolvedCaptureWorker, ResolvedWorker},
        _2_prepare_state,
        _5_finalize::identity::{publish, CoordinatorPin},
    },
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs::OpenOptions,
    io::Write,
    net::{Ipv4Addr, TcpListener},
    time::{Duration, Instant},
};
use svm_replay_protocol::{Digest, Error, PreparedRequest};
mod _0_debug;
use _0_debug as debug;

pub use debug::{execute, DebugExecution};

use debug::require;

pub(crate) use debug::execute_pinned;
mod _1_execute_inner;
use _1_execute_inner as execute_inner;

use execute_inner::execute_inner;

mod _2_native_gate;
use _2_native_gate as native_gate;
use native_gate::native_gate;

mod _3_capture_bounds;
use _3_capture_bounds as capture_bounds;
use capture_bounds::capture_bounds;
