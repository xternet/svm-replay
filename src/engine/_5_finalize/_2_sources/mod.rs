use crate::{_5_finalize, shared::history::History};
use serde_json::{json, Value};
use std::path::Path;
use svm_replay_protocol::Error;
pub(crate) fn record_sources(
    result: Result<Value, Error>,
    history: &History<'_>,
    job: &Path,
    receipt: &mut Value,
) -> Result<Value, Error> {
    let observations = json!(history.sources.observations());
    receipt["sourceDiagnostics"] = match history.sources.diagnostics() {
        Ok(value) => value,
        Err(error) => json!({"status":"ERROR","error":error}),
    };
    match _5_finalize::write_json(&job.join("source-observations.json"), &observations) {
        Ok(hash) => {
            receipt["sourceObservations"] =
                json!({"file":"source-observations.json","sha256":hash});
            result
        }
        Err(error) => Err(error.with_details(json!({"primaryError":result.err()}))),
    }
}
