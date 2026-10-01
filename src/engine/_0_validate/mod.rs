use svm_replay_protocol::{validate_safe_numbers, Error, PreparedRequest};

pub fn run(request: &PreparedRequest) -> Result<(), Error> {
    let value =
        serde_json::to_value(request).map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
    validate_safe_numbers(&value)?;
    request.validate()
}
