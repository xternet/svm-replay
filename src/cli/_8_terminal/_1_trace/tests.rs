use super::*;
use serde_json::json;
#[test]
fn shows_completeness_paths_and_call_nesting() {
    let text = summary(
        &json!({"receiptPath":"runs/job/receipt.json","trace":{"exports":[{
        "phase":"original-control","file":"trace-original-control.json","artifact":{"status":"TRUNCATED","reason":"byte budget"},
        "data":[{"event":{"kind":"enter","call_id":1,"invocation":{"program_id":"parent"}}},
            {"event":{"kind":"enter","call_id":2,"invocation":{"program_id":"child"}}}]}]}}),
    );
    assert!(text.contains("TRUNCATED"));
    assert!(text.contains("byte budget"));
    assert!(text.contains("trace-original-control.json"));
    assert!(text.contains("      #2 child"));
}
