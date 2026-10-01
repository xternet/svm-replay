//! Establish exact Bank-initialized context and emit the prepared boundary.
use super::*;
mod _0_derivation;
use _0_derivation as derivation;

pub(super) use derivation::Boundary;
mod _1_finish;
use _1_finish as finish;

pub(super) use finish::finish;

mod _2_resolve_sysvars;
use _2_resolve_sysvars as resolve_sysvars;
use resolve_sysvars::resolve_sysvars;

mod _3_initialize_sysvars;
use _3_initialize_sysvars as initialize_sysvars;
use initialize_sysvars::initialize_sysvars;

mod _4_reward_consistency;
