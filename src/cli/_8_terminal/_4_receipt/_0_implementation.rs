use super::*;

pub fn render(value: &Value, elapsed: Duration) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    if value["schema"] == "svm-historical-rpc/v1" && value["kind"] == "interrupted" {
        return writeln!(out, "\nHistorical fork stopped (interrupted).\n");
    }
    let passed = value["outcome"] == "COMPLETED";
    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let label = if value["schema"] == "svm-historical-rpc/v1" && value["kind"] == "closed" {
        "Historical fork stopped"
    } else if value["matched"] == true {
        "Replay matches the expected historical result"
    } else if passed {
        "Completed"
    } else {
        "Replay did not complete"
    };
    let prefix = if color {
        if passed {
            "\x1b[32m"
        } else {
            "\x1b[31m"
        }
    } else {
        ""
    };
    let reset = if color { "\x1b[0m" } else { "" };
    writeln!(
        out,
        "\n{prefix}{} {label}{reset} {}\n",
        if passed { "✓" } else { "✗" },
        style::stdout(&format!("({:.1}s)", elapsed.as_secs_f64()))
    )?;
    if let Some(warning) = value.pointer("/runtimeSelection/warning") {
        writeln!(out, "  Warning: {}", display(warning))?;
    }
    for (label, item) in [
        ("Outcome", &value["outcome"]),
        ("Category", &value["incomplete"]["category"]),
        ("Case", &value["caseId"]),
        ("Error", &value["error"]["code"]),
        ("Details", &value["error"]["message"]),
    ] {
        if !item.is_null() {
            writeln!(out, "  {label}: {}", style::stdout(&display(item)))?;
        }
    }
    if value.get("incomplete").is_some() {
        writeln!(out, "\n  Partial evidence only; no verified requested result or estimated state.\n  See incomplete.available in the JSON receipt for retained artifacts.")?;
    }
    if passed && value.get("receiptPath").is_some() && value.get("savedTo").is_none() {
        match result_summary(value) {
            Ok(summary) => writeln!(out, "{}", style::stdout(&summary))?,
            Err(error) => writeln!(
                out,
                "  Summary unavailable: {}. Inspect the receipt.",
                clean(&error.to_string())
            )?,
        }
    }
    if let Some(path) = value["receiptPath"].as_str() {
        let summary = trace::summary(value);
        if !summary.is_empty() {
            writeln!(out, "{}", style::stdout(&summary))?;
        }
        writeln!(out, "\nReceipt\n  {}", style::path(&clean(path)))?;
    }
    if let Some(path) = value["savedTo"].as_str() {
        writeln!(out, "\nSaved result\n  {}", style::path(&clean(path)))?;
    }
    if value["matched"] == true {
        writeln!(out, "\nTry your own transaction\n  svm-replay --tx SIGNATURE\n\n  Set API_ALCHEMY for historical data.\n  Use --help for replacement, overrides and trace options.")?;
    }
    Ok(())
}

pub(in super::super) fn result_summary(value: &Value) -> Result<String, Error> {
    use svm_replay_engine::shared::runtime::read_bounded_file;
    use svm_replay_protocol::{parse_json, Digest};
    let receipt = if value["matched"] == true {
        &value["receipt"]
    } else {
        value
    };
    if let Some(result) = receipt.get("result") {
        return Ok(format_result(result));
    }
    let path = receipt["receiptPath"]
        .as_str()
        .and_then(|p| std::path::Path::new(p).parent())
        .ok_or_else(|| Error::new("SUMMARY", "receipt path missing"))?;
    if receipt["output"]["file"] != "result.json" {
        return Err(Error::new("SUMMARY", "unexpected output path"));
    }
    let bytes = read_bounded_file(&path.join("result.json"), 16 * 1024 * 1024)
        .map_err(|e| Error::new("SUMMARY", e.to_string()))?;
    if Some(Digest::of(&bytes).as_str()) != receipt["output"]["sha256"].as_str() {
        return Err(Error::new("SUMMARY", "output hash differs"));
    }
    let result = parse_json(&bytes)?;
    Ok(format_result(&result))
}

pub(in super::super) fn format_result(result: &Value) -> String {
    let mut lines = Vec::new();
    for name in [
        "original",
        "replacement",
        "accountOverride",
        "requestedOverrides",
    ] {
        let tx = &result[name];
        if !tx.is_null() {
            transaction(&mut lines, &format!("Transaction ({name})"), tx);
        }
    }
    if let Some(what_if) = result.get("whatIf").filter(|value| !value.is_null()) {
        let runs = what_if["transactions"]
            .as_array()
            .map_or(&[][..], Vec::as_slice);
        let place = if what_if["mode"] == "after" {
            "after the original"
        } else {
            "instead of the original"
        };
        let overridden = what_if["stateOverrides"].as_array().map_or(0, Vec::len);
        lines.push(format!(
            "\nBundle: {} transaction(s) {place}; {overridden} account(s) overridden first",
            runs.len()
        ));
        for (position, tx) in runs.iter().enumerate() {
            let title = format!("Transaction (bundle {} of {})", position + 1, runs.len());
            transaction(&mut lines, &title, tx);
        }
    }
    lines.join("\n")
}

fn transaction(lines: &mut Vec<String>, title: &str, tx: &Value) {
    lines.push(format!("\n{title}"));
    for (field, label) in [("status", "Status"), ("computeUnits", "Compute units")] {
        if let Some(value) = tx.get(field) {
            lines.push(format!("  {label}: {}", display(value)));
        }
    }
    if let Some(logs) = tx["logs"].as_array().filter(|logs| !logs.is_empty()) {
        lines.push(format!("\nLogs ({}, showing up to 20)", logs.len()));
        for log in logs.iter().take(20) {
            lines.push(format!("  {}", display(log)));
        }
    }
    if let Some(accounts) = tx["accountTransitions"].as_array() {
        let changed: Vec<_> = accounts
            .iter()
            .filter(|a| a["before"] != a["after"])
            .collect();
        lines.push(format!(
            "\nAccount changes ({}, showing up to 3)",
            changed.len()
        ));
        for account in changed.iter().take(3) {
            lines.push(format!(
                "\n  {}\n    Lamports: {} → {}",
                display(&account["pubkey"]),
                display(&account["before"]["lamports"]),
                display(&account["after"]["lamports"])
            ));
            if !account["before"]["tokenAmount"].is_null()
                || !account["after"]["tokenAmount"].is_null()
            {
                lines.push(format!(
                    "    Tokens (raw): {} → {}",
                    display(&account["before"]["tokenAmount"]),
                    display(&account["after"]["tokenAmount"])
                ));
            }
        }
        if changed.len() > 3 {
            lines.push("\n  Full changes are in the receipt's result artifact.".into());
        }
    }
}
