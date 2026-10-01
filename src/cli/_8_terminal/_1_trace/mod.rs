//! Bounded text view of verified exports, not a second debugger.
use super::display;
use serde_json::Value;

pub(super) fn summary(receipt: &Value) -> String {
    let Some(exports) = receipt.pointer("/trace/exports").and_then(Value::as_array) else {
        return String::new();
    };
    let mut lines = vec!["\nCall traces (first 40 calls per export)".into()];
    for export in exports {
        lines.push(format!(
            "\n  {}: {}",
            display(&export["phase"]),
            display(&export["artifact"]["status"])
        ));
        if !export["artifact"]["reason"].is_null() {
            lines.push(format!(
                "  Reason: {}",
                display(&export["artifact"]["reason"])
            ));
        }
        if let (Some(receipt_path), Some(file)) =
            (receipt["receiptPath"].as_str(), export["file"].as_str())
        {
            if let Some(parent) = std::path::Path::new(receipt_path).parent() {
                lines.push(format!(
                    "  Export: {}",
                    super::clean(&parent.join(file).display().to_string())
                ));
            }
        }
        if let Some(events) = export["data"].as_array() {
            let mut depth: usize = 0;
            let mut calls = 0;
            for row in events {
                let event = &row["event"];
                match event["kind"].as_str() {
                    Some("enter" | "enter_omitted") => {
                        calls += 1;
                        if calls <= 40 {
                            lines.push(format!(
                                "    {}#{} {}",
                                "  ".repeat(depth.min(8)),
                                display(&event["call_id"]),
                                display(&event["invocation"]["program_id"])
                            ));
                        }
                        depth += 1;
                    }
                    Some("exit" | "exit_omitted") => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
            if calls > 40 {
                lines.push(format!("  {} further calls in the export", calls - 40));
            }
        }
    }
    lines.join("\n")
}

#[path = "tests.rs"]
#[cfg(test)]
mod tests;
