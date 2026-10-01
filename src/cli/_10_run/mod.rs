use super::{arguments, debugger, terminal};

pub(super) fn run() -> std::process::ExitCode {
    let start = std::time::Instant::now();
    let value = match arguments::run() {
        Ok(value) => value,
        Err(error) => arguments::error_receipt(error),
    };
    let status = match value.get("outcome").and_then(serde_json::Value::as_str) {
        Some("COMPLETED") => std::process::ExitCode::SUCCESS,
        _ => std::process::ExitCode::FAILURE,
    };
    if value["schema"] == "svm-replay-help/v1" {
        return status;
    }
    let output = if terminal::enabled() {
        terminal::render(&value, start.elapsed()).map_err(|e| e.to_string())
    } else {
        debugger::write_final(&value).map_err(|e| e.to_string())
    };
    if let Err(error) = output {
        eprintln!("OUTPUT_ERROR: {error}");
        if let Some(path) = value.get("receiptPath").and_then(serde_json::Value::as_str) {
            eprintln!("Receipt saved at {path}");
        }
        return std::process::ExitCode::FAILURE;
    }
    status
}
