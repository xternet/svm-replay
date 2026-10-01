use super::*;

mod _0_implementation;
#[cfg(not(any(unix, windows)))]
pub use _0_implementation::run;
#[cfg(not(any(unix, windows)))]
pub(crate) use _0_implementation::write_final;
#[cfg(test)]
pub(super) use _0_implementation::MAX_LINE;
#[cfg(any(unix, windows))]
pub(super) use _0_implementation::{Input, MAX_STREAM};
