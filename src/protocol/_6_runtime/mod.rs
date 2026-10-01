//! Internal integrity of an explicitly reviewed runtime binding, not era discovery.
use crate::{Digest, Error};
use serde_json::Value;
mod _0_binding;
use _0_binding as binding;

pub use binding::validate_runtime_binding;
