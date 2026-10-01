//! Guarded, recorded reconstruction of absent ordinary Program headers.
use super::*;
use crate::_1_resolve_runtime::registry::Registry;
use crate::_2_prepare_state::analysis::Analysis;
use crate::shared::{
    bank::program_header,
    history::{account_data, data, program_data_address},
};
mod _0_coordinator;
mod _1_acquire;
mod _2_evidence;
use _1_acquire::recover;
use _2_evidence::{anchor_slots, observe, preexisting};
#[cfg(test)]
mod tests;

pub(super) struct Recovery<'a> {
    a: &'a Analysis,
    block: &'a Value,
    binding: &'a Value,
    programs: BTreeSet<String>,
    images: BTreeMap<String, Value>,
    pub(super) proofs: Vec<Value>,
}

fn unsupported(message: impl Into<String>) -> Error {
    Error::new("UNSUPPORTED_PROGRAM_HEADER_EVIDENCE", message)
}
