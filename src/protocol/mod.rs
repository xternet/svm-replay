//! Versioned, runtime-neutral replay contracts. No provider or worker IO.
mod _0_historical;
use _0_historical as historical;
mod _1_identity;
use _1_identity as identity;
mod _2_json;
use _2_json as json;
mod _3_request;
use _3_request as request;
pub mod _4_transaction;
pub use _4_transaction as transaction;
pub mod _5_worker;
pub use _5_worker as worker;

pub use historical::HistoricalRequest;
pub use identity::Digest;
pub use json::{parse_json, validate_safe_numbers};
pub use request::{Limits, MetadataPolicy, PreparedRequest};

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct Error {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl Error {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
pub mod _6_runtime;
pub use _6_runtime as runtime;
