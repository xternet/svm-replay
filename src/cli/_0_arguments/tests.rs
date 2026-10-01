use super::*;

#[test]
fn signature_flags_combine_without_manual_runtime_arguments() {
    let args = Args::try_parse_from([
        "svm-replay",
        "--tx",
        "signature",
        "--replace",
        "@/tmp/tx.json",
        "--overrides",
        "[]",
        "--collect",
        r#"{"calls":true}"#,
        "--fields",
        "status,logs,computeUnits",
        "--out",
        "results",
        "--no-cache",
        "--alchemy-key-file",
        "key.txt",
        "--human",
    ])
    .unwrap();
    assert_eq!(args.signature.tx.as_deref(), Some("signature"));
    assert!(args.command.is_none());
    for arguments in [
        vec!["svm-replay", "--collect", "{}"],
        vec!["svm-replay", "--replace", "{}"],
        vec!["svm-replay", "--tx", "signature", "--demo"],
    ] {
        assert!(Args::try_parse_from(arguments).is_err());
    }
}

#[test]
fn bug_report_flag_is_optional_and_global() {
    assert!(Args::try_parse_from(["svm-replay", "demo"])
        .unwrap()
        .bug_report
        .is_none());
    for argv in [
        vec!["svm-replay", "--bug-report", "report.md", "demo"],
        vec!["svm-replay", "demo", "--bug-report", "report.md"],
    ] {
        assert_eq!(
            Args::try_parse_from(argv).unwrap().bug_report,
            Some(PathBuf::from("report.md"))
        );
    }
}
