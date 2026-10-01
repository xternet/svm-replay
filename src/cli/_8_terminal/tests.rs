use super::*;
#[test]
fn selected_result_does_not_reread_unfiltered_artifact() {
    let summary =
        result_summary(&serde_json::json!({"result":{"original":{"status":"ok"}}})).unwrap();
    assert!(summary.contains("Status: ok"));
    assert!(!summary.contains("Compute units"));
    assert!(!summary.contains("null"));
}
#[test]
fn semantic_colors_and_plain_output() {
    let line = "✓ COMPLETED true false 150 /tmp/result.json";
    assert_eq!(style::highlight(line, false), line);
    let colored = style::highlight(line, true);
    for expected in [
        "\x1b[32m✓",
        "\x1b[32mCOMPLETED",
        "\x1b[35mtrue",
        "\x1b[35mfalse",
        "\x1b[33m150",
        "\x1b[34m/tmp/result.json",
    ] {
        assert!(colored.contains(expected), "{colored:?}");
    }
    assert!(style::highlight(&"a".repeat(64), true).starts_with("\x1b[36m"));
    assert!(style::highlight("✗ ERROR UNSUPPORTED", true).contains("\x1b[31m✗"));
}
#[test]
fn menu_selection_has_own_line() {
    use dialoguer::theme::Theme;
    let mut text = String::new();
    style::MenuTheme
        .format_select_prompt_selection(
            &mut text,
            "Choose an offline example",
            "Bundled historical replay",
        )
        .unwrap();
    assert_eq!(
        text,
        "Choose an offline example:\n> Bundled historical replay\n"
    );
}
#[test]
fn spinner_completion_requires_explicit_success() {
    assert_eq!(
        progress_end(Some("Inputs verified"), Duration::from_secs(2)),
        "✓ Inputs verified (2.0s)"
    );
    let interrupted = progress_end(None, Duration::ZERO);
    assert!(interrupted.starts_with("✗"));
    assert!(!interrupted.contains('✓'));
}
#[test]
fn progress_depends_on_stderr_not_json_output() {
    assert!(animate_progress(true, false));
    assert!(!animate_progress(false, false));
    assert!(!animate_progress(true, true));
    assert!(!animate_progress(false, true));
}
#[test]
fn summary_separates_sections_and_omits_empty_tokens() {
    let text = format_result(
        &serde_json::json!({"original":{"status":"ok","computeUnits":123,"accountTransitions":[{"pubkey":"example","before":{"lamports":"10","tokenAmount":null},"after":{"lamports":"9","tokenAmount":null}}]}}),
    );
    assert!(text.contains("\nTransaction (original)\n  Status: ok\n"));
    assert!(text.contains("\n\n  example\n    Lamports: 10 → 9"));
    assert!(!text.contains("null"));
    assert!(!text.contains('"'));
}
#[test]
fn strips_terminal_controls() {
    assert_eq!(clean("a\x1b\n\rb"), "ab");
}
#[test]
fn bounds_display_text() {
    assert_eq!(clean(&"x".repeat(1000)).len(), 500);
}
