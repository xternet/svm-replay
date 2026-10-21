use super::{arguments, debugger, terminal};

pub(super) fn run() -> std::process::ExitCode {
    let start = std::time::Instant::now();
    let (result, progress) = arguments::run();
    let value = match result {
        Ok(value) => value,
        Err(error) => arguments::error_receipt(error),
    };
    let status = match value.get("outcome").and_then(serde_json::Value::as_str) {
        Some("COMPLETED") => std::process::ExitCode::SUCCESS,
        Some("CANCELLED") if value["schema"] == "svm-historical-rpc/v1" => {
            std::process::ExitCode::from(130)
        }
        _ => std::process::ExitCode::FAILURE,
    };
    if matches!(
        value["schema"].as_str(),
        Some("svm-replay-help/v1" | "svm-replay-completions/v1")
    ) {
        if let Some(progress) = progress {
            progress.finish("COMPLETED");
        }
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
        if let Some(progress) = progress {
            progress.finish("ERROR");
        }
        return std::process::ExitCode::FAILURE;
    }
    if let Some(progress) = progress {
        progress.finish(value["outcome"].as_str().expect("CLI outcome required"));
    }
    status
}
