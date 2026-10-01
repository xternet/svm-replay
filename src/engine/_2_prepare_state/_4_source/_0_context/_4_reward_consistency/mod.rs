//! Reject historical sysvar images contradicted by reviewed Bank timing.
use super::*;

#[cfg(test)]
mod tests;

mod _0_validation;
pub(super) use _0_validation::validate;

mod _1_terminal;
mod _2_acquire;
mod _3_fields;
pub(super) use _2_acquire::{inactive_evidence, repair};
