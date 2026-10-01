use super::*;
pub(in super::super::super) fn json_argument(input: &str) -> Result<Value, Error> {
    let bytes = if let Some(path) = input.strip_prefix('@') {
        if path.is_empty() {
            return Err(Error::new("INVALID_REQUEST", "@ requires a file path"));
        }
        read_bounded_file(Path::new(path), 16 * 1024 * 1024)
            .map_err(|e| Error::new("INPUT_IO", e.to_string()))?
    } else {
        input.as_bytes().to_vec()
    };
    parse_json(&bytes)
}
pub(in super::super::super) fn replacement(input: &str) -> Result<String, Error> {
    let value = json_argument(input)?;
    let object = value
        .as_object()
        .filter(|v| v.len() == 1 && v.contains_key("transactionBase64"))
        .ok_or_else(|| {
            Error::new(
                "INVALID_REQUEST",
                "replacement requires only transactionBase64",
            )
        })?;
    let wire = object["transactionBase64"]
        .as_str()
        .ok_or_else(|| Error::new("INVALID_REQUEST", "transactionBase64 must be a string"))?;
    svm_replay_protocol::transaction::decode(wire)?;
    Ok(wire.to_owned())
}
pub(in super::super::super) fn overrides(input: &str) -> Result<Value, Error> {
    let value = json_argument(input)?;
    let patches =
        svm_replay_engine::shared::dependencies::overrides::validate_requested_overrides(&value)?;
    for patch in patches {
        svm_replay_engine::shared::history::address(&patch.pubkey, 32)?;
    }
    Ok(value)
}
