use super::*;
use std::path::Path;

fn refusal(code: &str, message: &str) -> Error {
    Error::new(code, message)
}

/// Whether the embedder's store and the journal agree before anything starts: a store with
/// history (`existing`) needs its journal, and one without (`new`) must have none.
pub fn require_journal(path: &Path, require: Option<&str>) -> Result<(), Error> {
    match (require, path.exists()) {
        (Some("existing"), false) => Err(refusal(
            "JOURNAL_MISSING",
            "the surfnet's store has history but its journal is missing",
        )),
        (Some("new"), true) => Err(refusal(
            "JOURNAL_STORE_MISMATCH",
            "a journal exists for a surfnet store that has no history",
        )),
        _ => Ok(()),
    }
}

/// The retained inputs a journal's session was first opened from, for a restart to reopen
/// exactly. It must have been started from the same `--after` selector (checked before
/// anything is prepared), and its initial receipt must still have its recorded digest.
pub fn recorded_input(path: &Path, after: Option<&str>) -> Result<Option<Value>, Error> {
    if !path.exists() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(path).map_err(|e| Error::new("JOURNAL_IO", e.to_string()))?;
    let header: Value = text
        .lines()
        .next()
        .and_then(|line| serde_json::from_str(line).ok())
        .ok_or_else(|| refusal("JOURNAL_CORRUPT", "journal header unreadable"))?;
    if header["after"] != json!(after) {
        return Err(refusal(
            "JOURNAL_BOUNDARY_MISMATCH",
            "the journal was started from another --after boundary",
        ));
    }
    let input = header["input"].clone();
    let receipt = input["receiptPath"]
        .as_str()
        .ok_or_else(|| refusal("JOURNAL_CORRUPT", "journal input lacks its receipt"))?;
    let retained = crate::shared::runtime::file_sha256(Path::new(receipt)).map_err(|_| {
        refusal(
            "JOURNAL_RECEIPT_MISSING",
            "the initial receipt is no longer retained",
        )
    })?;
    if input["receiptSha256"] != json!(retained) {
        return Err(refusal(
            "JOURNAL_RECEIPT_CHANGED",
            "the retained initial receipt changed",
        ));
    }
    Ok(Some(input))
}
