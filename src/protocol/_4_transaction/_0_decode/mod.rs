use super::*;

mod _0_implementation;

pub use _0_implementation::{assert_historical, decode};
pub(super) use _0_implementation::{invalid, Reader};

#[cfg(test)]
mod tests;
