//! Readable help for the canonical commands and retained compatibility options.
//! clap drops colors when output is not a terminal or NO_COLOR is set.
use super::*;

mod _0_implementation;

pub(super) use _0_implementation::{command, LIMITS, OUTPUT, PINNED, PROVIDER, REPLAY, WHAT_IF};
