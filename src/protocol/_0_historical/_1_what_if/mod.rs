//! What-if requests: transactions run after or instead of the target, on eth_call-style state
//! overrides (any account, existing or new). The original is still replayed and checked first.
use super::*;

mod _0_implementation;

pub use _0_implementation::{StateOverride, WhatIf, WhatIfMode, MAX_BUNDLE, MAX_STATE_OVERRIDES};
