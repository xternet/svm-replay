use super::_0_implementation::write_ready;
use super::*;

fn ready() -> Value {
    json!({"schema":"svm-historical-rpc/v1","kind":"ready",
        "url":"http://127.0.0.1:18899","wsUrl":"ws://127.0.0.1:18900",
        "pins":{"slot":413023261,"transactionIndex":6,
            "runtime":{"name":"Agave 3.1.11"},
            "worker":{"sourceSha256":"internal pin"},
            "sourceEvidenceHashes":vec!["internal evidence";1000]}})
}

#[test]
fn human_ready_is_compact_and_explains_foreground_server() {
    let mut output = Vec::new();
    write_ready(&mut output, &ready(), true).unwrap();
    let text = String::from_utf8(output).unwrap();
    for expected in [
        "Historical fork ready",
        "http://127.0.0.1:18899",
        "ws://127.0.0.1:18900",
        "413023261",
        "Agave 3.1.11",
        "frozen",
        "Ctrl-C",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert!(!text.contains("sourceSha256"));
    assert!(!text.contains("internal evidence"));
    assert!(text.len() < 800, "{} bytes", text.len());
    assert!(text.contains("\n\nServing requests.\nPress Ctrl-C to stop.\n"));
    assert!(text.contains("\n\n  Clock/slot frozen"));
    assert!(text.lines().count() <= 14);
}

#[test]
fn machine_ready_preserves_the_complete_sdk_handshake() {
    let expected = ready();
    let mut output = Vec::new();
    write_ready(&mut output, &expected, false).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&output).unwrap(), expected);
    assert_eq!(output.iter().filter(|b| **b == b'\n').count(), 1);
}

#[test]
fn human_ready_warns_about_unqualified_runtime_and_cleans_controls() {
    let mut ready = ready();
    ready["pins"]["runtimeSelection"] = json!({"warning":"Unqualified newer slot\x1b[2J"});
    let mut output = Vec::new();
    write_ready(&mut output, &ready, true).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Warning: Unqualified newer slot"));
    assert!(!text.contains("\x1b[2J"));
}
