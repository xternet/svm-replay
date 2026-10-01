use super::*;

#[test]
fn documented_collection_configuration_is_valid() {
    let guide = include_str!("../../../../../../docs/tracing.md");
    let sample = guide
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let options = collect(sample).unwrap().unwrap();
    assert_eq!(options.capture.sbpf_observations, "pc-registers-memory");
}

#[test]
fn collection_defaults_dependencies_and_limits() {
    let all = collect("all").unwrap().unwrap();
    assert_eq!(all.capture.sbpf_observations, "pc-registers-memory");
    assert!(all.require_complete);
    assert_eq!(all.bounds.memory_rows, Some(2_000_000));
    assert_eq!(all.bounds.register_rows, 2_000_000);
    assert_eq!(all.capture.limits.max_events, 1_000_000);
    assert_eq!(all.capture.limits.max_bytes, 256 * 1024 * 1024);
    assert!(collect("{}").unwrap().is_none());
    assert!(collect(r#"{"calls":false}"#).unwrap().is_none());
    let calls = collect(r#"{"calls":true}"#).unwrap().unwrap();
    assert_eq!(calls.capture.level, "calls");
    assert_eq!(calls.bounds.register_rows, 0);
    for key in ["instructions", "registers", "memory"] {
        let options = collect(&format!(r#"{{"{key}":true}}"#)).unwrap().unwrap();
        assert_eq!(options.capture.level, "sbpf");
        assert!(options.bounds.register_rows > 0);
        assert_eq!(options.bounds.memory_rows.is_some(), key == "memory");
    }
    for invalid in [
        r#"{"typo":true}"#,
        r#"{"calls":"yes"}"#,
        r#"{"calls":true,"maxBytes":0}"#,
        r#"{"calls":true,"maxEvents":1000001}"#,
        r#"{"programIds":["bad"]}"#,
    ] {
        assert!(collect(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn collection_file_filters_and_all_enabled() {
    let json = r#"{"calls":true,"instructions":true,"registers":true,"memory":true,
        "programIds":["11111111111111111111111111111111"],"instructionIndices":[0],
        "maxBytes":1048576,"maxEvents":500,"timeoutMs":1000,
        "registerRows":25,"memoryRows":10,"maxInvocations":4}"#;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("collect.json");
    std::fs::write(&file, json).unwrap();
    let direct = collect(json).unwrap().unwrap();
    let from_file = collect(&format!("@{}", file.display())).unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&direct).unwrap(),
        serde_json::to_value(from_file).unwrap()
    );
    assert_eq!(direct.capture.filter.instruction_indices, vec![0]);
    assert_eq!(direct.bounds.memory_rows, Some(10));
    assert!(!direct.require_complete);
    assert_eq!(direct.capture.limits.max_events, 500);
}

#[test]
fn only_explicit_collection_limits_allow_partial_results() {
    for input in [
        "all",
        r#"{"memory":true}"#,
        r#"{"calls":true,"instructionIndices":[0]}"#,
    ] {
        assert!(collect(input).unwrap().unwrap().require_complete);
    }
    for field in [
        "maxBytes",
        "maxEvents",
        "timeoutMs",
        "registerRows",
        "memoryRows",
        "maxInvocations",
    ] {
        let input = format!(r#"{{"memory":true,"{field}":10}}"#);
        assert!(!collect(&input).unwrap().unwrap().require_complete);
    }
}
