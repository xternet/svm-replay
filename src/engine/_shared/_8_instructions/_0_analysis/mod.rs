use super::*;

#[cfg(test)]
mod tests;

mod _0_implementation;

pub use _0_implementation::{
    classify, is_unexecuted_load_failure, requirements, resolve, Instruction, SLOT_HASHES,
    SLOT_HISTORY,
};

pub(super) use _0_implementation::bad;
