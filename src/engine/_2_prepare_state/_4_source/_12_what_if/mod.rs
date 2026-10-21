//! Requested transactions at the target position: one replacement (0.1.0 requests) or a what-if
//! bundle. Their accounts and every state-override account join the dependency closure, so
//! same-block predecessors that write them replay first.
use super::*;
use crate::_2_prepare_state::analysis::Analysis;

mod _0_closure;
use _0_closure as closure;
mod _1_fixture;
use _1_fixture as fixture;

pub(super) use closure::{
    dependency_target, merged_summary, override_programs, requested_transactions,
};
pub(super) use fixture::{coverage_hint, insert_variants, inventory_images};

#[cfg(test)]
mod tests;
