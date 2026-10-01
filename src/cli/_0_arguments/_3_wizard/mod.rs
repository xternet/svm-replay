//! Collect existing CLI arguments, never execute a generated shell command.
use super::*;
use dialoguer::Input;

#[path = "tests.rs"]
#[cfg(test)]
mod tests;

mod _0_implementation;
#[cfg(test)]
use _0_implementation::{command, quote};

pub(super) use _0_implementation::run;
