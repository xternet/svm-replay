//! Exact alternate archive indexes, never current-state or estimated recovery.
use super::*;
mod _0_implementation;
pub(super) use _0_implementation::{
    block_request, block_wire, missing, status_request, status_slot, validate_location,
    validate_shape,
};
