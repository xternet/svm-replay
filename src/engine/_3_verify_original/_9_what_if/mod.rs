//! What-if runs: the bundle's results must match the request one to one, and every state
//! override must be visible in the account before the first transaction.
use super::*;

mod _0_implementation;

pub(super) use _0_implementation::what_if_proof;
