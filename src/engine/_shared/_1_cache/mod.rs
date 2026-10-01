//! Pure cache identity and checkpoint validation; replay ordering lives in workflow.
pub mod _0_boundary;
pub use _0_boundary as boundary;
pub mod _1_identity;
pub use _1_identity as identity;
