use crate::_5_finalize;
use serde_json::{json, Value};
use std::path::Path;
use svm_replay_protocol::Error;
pub(crate) fn archive_failed_attempt(
    job: &Path,
    receipt: &mut Value,
    ordinal: usize,
    error: &mut Error,
) -> Result<(), Error> {
    let disposition = if error.code == "NEEDS_INPUT" {
        "DISCARDED_NEEDS_INPUT"
    } else {
        "FAILED"
    };
    let archived = json!({"phases":receipt["phases"],"cache":receipt.get("cache"),"trace":receipt.get("trace"),
        "controlVerification":receipt["controlVerification"],"controlEvidence":receipt.get("controlEvidence"),
        "error":error,"disposition":disposition,"partialResultPublished":false});
    if error.code == "NEEDS_INPUT" {
        // Historical proof remains auditable for this exact abandoned fixture,
        // but cannot certify a newly hydrated attempt or become a public result.
        receipt["controlVerification"] = Value::Null;
        receipt["verification"] = Value::Null;
        receipt
            .as_object_mut()
            .ok_or_else(|| Error::new("INTERNAL", "receipt object changed"))?
            .remove("controlEvidence");
        if let Some(details) = error.details.as_mut().and_then(Value::as_object_mut) {
            details.remove("controlEvidence");
        }
    }
    let file = format!("attempt-{ordinal}.json");
    let sha = _5_finalize::write_json(&job.join(&file), &archived).map_err(|archive_error| {
        archive_error.with_details(json!({"archiveFile":file,"failedAttempt":archived}))
    })?;
    if receipt.get("failedAttempts").is_none() {
        receipt["failedAttempts"] = json!([]);
    }
    receipt["failedAttempts"]
        .as_array_mut()
        .ok_or_else(|| Error::new("INTERNAL", "attempt list changed"))?
        .push(json!({"ordinal":ordinal,"file":file,"sha256":sha,"disposition":disposition}));
    Ok(())
}
