//! Exact credit-only field-invariance proofs; never fills an absent image.
mod _0_proof;
pub use _0_proof::prove;
mod _1_plan;
pub use _1_plan::{plan, CreditPlan};
mod _2_binding;
pub use _2_binding::{supports_credit_preloads, validate_preloads};
#[cfg(test)]
mod tests;
