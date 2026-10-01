use super::*;

mod _0_implementation;

pub use _0_implementation::{
    Result, SemanticClosureResult, SemanticTransaction, WritableIntersection, VOTE_PROGRAM,
};

pub(super) use _0_implementation::{
    invalid, pubkey, safe_integer, unique, unique_sorted, EXCLUDED,
};
