use super::*;

mod _0_implementation;

pub(in super::super) use _0_implementation::{check, key, shape};

pub(super) use _0_implementation::member;

pub use _0_implementation::{CaptureFilter, CaptureLimits, CaptureRequest, ProducerBounds};
