//! Native final-write transformation; caller must bind provider and runtime evidence.
use super::*;
mod _0_implementation;
pub(super) use _0_implementation::{recover, TerminalContext};
#[cfg(test)]
mod tests;
