use super::*;

#[test]
fn source_network_is_bound_even_before_any_guarded_read() {
    let capture = Capture::new(&[], identity("capture", false));
    let source = CompositeSource::new(vec![Box::new(capture.open().unwrap())]).unwrap();
    source.assert_genesis(&key(1)).unwrap();
    assert_eq!(
        source.assert_genesis(&key(9)).unwrap_err().code,
        "SOURCE_CONTEXT_MISMATCH"
    );
    assert!(source.observations().is_empty());
    assert_eq!(
        source.diagnostics().unwrap()["sources"][0]["diagnostics"]["status"],
        "LOCAL_ONLY"
    );
    assert_eq!(
        source.diagnostics().unwrap()["sources"][0]["diagnostics"]["transport"]["requests"],
        0
    );
}

#[test]
fn custom_source_diagnostics_never_infer_zero_network_from_retained_data() {
    let source = CompositeSource::new(vec![controlled(
        "custom",
        account(12, "1"),
        Arc::new(AtomicUsize::new(0)),
        false,
    )])
    .unwrap();
    let diagnostics = source.diagnostics().unwrap();
    assert_eq!(
        diagnostics["sources"][0]["diagnostics"],
        json!({"status":"NOT_REPORTED"})
    );
}

#[test]
fn captured_restart_preserves_u64_strings_and_all_hashes() {
    let row = account(12, "9007199254740993");
    let capture = Capture::new(&[row.clone()], identity("capture", false));
    let digest = capture.write();
    let source = capture.open().expect("open");
    let first = source
        .inspect(&row["query"])
        .expect("inspect")
        .expect("available");
    assert_eq!(first["value"], row["value"]);
    let evidence = first["evidenceHashes"].as_array().expect("hashes");
    for expected in [
        json!(digest),
        capture.manifest["entries"][0]["sha256"].clone(),
        row["evidenceHashes"][0].clone(),
    ] {
        assert!(evidence.contains(&expected));
    }
    assert_eq!(
        CapturedSource::open(&capture.path(), &digest)
            .expect("restart")
            .inspect(&row["query"])
            .expect("inspect"),
        Some(first)
    );
}

#[test]
fn explicit_absence_is_available_but_missing_input_is_not() {
    let mut row = account(12, "1");
    row["value"] =
        json!({"pubkey":key(2),"sourceSlot":12,"role":"application","presence":"absent"});
    let capture = Capture::new(&[row.clone()], identity("capture", false));
    let mut source =
        CompositeSource::new(vec![Box::new(capture.open().expect("open"))]).expect("composite");
    assert_eq!(
        source.require(&row["query"]).expect("explicit absence")["value"],
        row["value"]
    );
    let missing = account(13, "1")["query"].clone();
    assert!(source.inspect(&missing).expect("missing").is_none());
    let error = source.require(&missing).expect_err("unavailable");
    assert_eq!(error.code, "SOURCE_UNAVAILABLE");
    let details = error.details.expect("query details");
    assert_eq!(
        details["observations"]
            .as_array()
            .expect("observations")
            .len(),
        1
    );
    assert_eq!(details["observations"][0]["query"]["slot"], 13);
    assert_eq!(
        source.observations().last().expect("observation")["status"],
        "UNAVAILABLE"
    );
}

#[test]
fn coverage_has_no_fixed_history_age_and_does_not_invent_entries() {
    let rows = [account(1, "1"), account(999_999_999, "1")];
    let capture = Capture::new(&rows, identity("capture", false));
    let source = capture.open().expect("open");
    for row in rows {
        assert_eq!(
            source
                .inspect(&row["query"])
                .expect("inspect")
                .expect("available")["value"],
            row["value"]
        );
    }
    assert!(source
        .inspect(&account(1_000_000_001, "1")["query"])
        .expect("outside coverage")
        .is_none());
}

#[test]
fn manifest_is_pinned_and_record_digest_checked_on_every_read() {
    let row = account(12, "1");
    let capture = Capture::new(&[row.clone()], identity("capture", false));
    let source = capture.open().expect("open");
    assert_eq!(
        CapturedSource::open(&capture.path(), &Digest::of("wrong"))
            .err()
            .expect("bad hash")
            .code,
        "SOURCE_INTEGRITY"
    );
    std::fs::write(capture.dir.path().join("record-0.json"), b"changed")
        .expect("tamper exact test file");
    assert_eq!(
        source.inspect(&row["query"]).expect_err("corruption").code,
        "SOURCE_INTEGRITY"
    );
}

#[test]
fn manifest_paths_cannot_escape_or_be_ambiguous() {
    for file in [
        "../outside.json",
        "/tmp/outside.json",
        "./record-0.json",
        "a//b",
        "a\\b",
        "",
    ] {
        let mut capture = Capture::new(&[account(12, "1")], identity("capture", false));
        capture.manifest["entries"][0]["file"] = json!(file);
        assert_eq!(
            capture.open().err().expect("invalid path").code,
            "INVALID_SOURCE",
            "{file}"
        );
    }
}

#[cfg(unix)]
#[test]
fn external_symlink_rejects_even_when_bytes_match() {
    let row = account(12, "1");
    let mut capture = Capture::new(&[row.clone()], identity("capture", false));
    let outside = tempfile::tempdir().expect("outside fixture");
    std::fs::copy(
        capture.dir.path().join("record-0.json"),
        outside.path().join("copy.json"),
    )
    .expect("copy fixture");
    std::os::unix::fs::symlink(
        outside.path().join("copy.json"),
        capture.dir.path().join("link.json"),
    )
    .expect("test symlink");
    capture.manifest["entries"][0]["file"] = json!("link.json");
    assert_eq!(
        capture
            .open()
            .expect("manifest valid")
            .inspect(&row["query"])
            .expect_err("escaped symlink")
            .code,
        "INVALID_SOURCE"
    );
}
