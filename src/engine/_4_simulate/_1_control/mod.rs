//! Original historical proof is independent of later requested execution failure.
use crate::shared::diff::canonical_json;
use serde_json::{json, Value};
use svm_replay_protocol::{Digest, Error};

#[cfg(test)]
mod tests;

/// Failed control output is diagnostic evidence, never a verified replay result.
pub(crate) fn retain_failure(
    job: &std::path::Path,
    output: &Value,
    message: String,
) -> Result<Error, Error> {
    let file = "unverified-control-output.json";
    let bytes =
        serde_json::to_vec(output).map_err(|error| Error::new("OUTPUT_JSON", error.to_string()))?;
    let hash = crate::_5_finalize::write_bytes(&job.join(file), &bytes)?;
    Ok(Error::new("MISMATCH", message)
        .with_details(json!({"unverifiedOutput":{"file":file,"sha256":hash,"verified":false}})))
}

pub(crate) fn evidence(fixture: &Value, output: &Value, verification: &Value) -> Value {
    json!({"schema":"svm-original-control-evidence/v1","phase":"original-control",
        "fixtureSha256":Digest::of(canonical_json(fixture)),"outputSha256":Digest::of(canonical_json(output)),
        "verification":verification,"complete":true})
}

pub(crate) fn attach(mut error: Error, evidence: Option<&Value>) -> Error {
    if let Some(evidence) = evidence {
        let mut details = match error.details.take() {
            Some(value) if value.is_object() && value.get("schema").is_none() => value,
            Some(value) => json!({"cause":value}),
            None => json!({}),
        };
        details["controlEvidence"] = evidence.clone();
        error.details = Some(details);
    }
    error
}

/// Only direct current-attempt evidence is promoted, never nested discarded errors.
pub(crate) fn preserve(receipt: &mut Value, error: &Error) {
    if let Some(evidence) = error
        .details
        .as_ref()
        .and_then(|v| v.get("controlEvidence"))
    {
        receipt["controlVerification"] = evidence["verification"].clone();
        receipt["controlEvidence"] = evidence.clone();
    }
}
