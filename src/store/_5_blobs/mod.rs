use super::*;

mod _0_implementation;
#[cfg(windows)]
mod _1_windows;

pub(super) use _0_implementation::{collect_garbage, evict, publish_blob, read_blob};
