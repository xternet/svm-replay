use serde_json::{json, Value};
use svm_replay_protocol::Error;

/// Pre-execution errors have no durable engine receipt.
pub(crate) fn error_receipt(error: Error) -> Value {
    let outcome = if error.code.starts_with("UNSUPPORTED_")
        || matches!(
            error.code.as_str(),
            "SOURCE_UNAVAILABLE" | "CAPABILITY_UNAVAILABLE"
        ) {
        "UNSUPPORTED"
    } else if error.code == "NEEDS_INPUT" {
        "NEEDS_INPUT"
    } else if matches!(error.code.as_str(), "CANCELLED" | "WORKER_CANCELLED") {
        "CANCELLED"
    } else if matches!(error.code.as_str(), "SOURCE_DEADLINE" | "WORKER_TIMEOUT") {
        "TIMEOUT"
    } else {
        "ERROR"
    };
    json!({"schema":"svm-replay-cli-error/v1","outcome":outcome,"error":error})
}
