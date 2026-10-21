//! Which accounts the closure fetches for the requested transactions and state overrides.
use super::*;

mod _0_implementation;

pub(in super::super) use _0_implementation::{
    dependency_target, merged_summary, override_programs, requested_transactions,
};
