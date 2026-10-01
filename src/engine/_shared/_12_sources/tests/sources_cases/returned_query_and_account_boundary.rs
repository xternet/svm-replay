use super::*;

#[test]
fn archived_write_record_never_substitutes_for_end_slot() {
    let mut row = account(12, "1");
    row["query"]["phase"] = json!("last-write-at-or-before-slot");
    row["value"]["sourceSlot"] = json!(11);
    let capture = Capture::new(&[row.clone()], identity("capture", false));
    let source = capture.open().unwrap();
    assert!(source.inspect(&row["query"]).unwrap().is_some());
    let mut end_slot = row["query"].clone();
    end_slot["phase"] = json!("end-slot");
    assert!(source.inspect(&end_slot).unwrap().is_none());
    row["value"]["sourceSlot"] = json!(13);
    let capture = Capture::new(&[row.clone()], identity("capture", false));
    assert_eq!(
        capture
            .open()
            .unwrap()
            .inspect(&row["query"])
            .unwrap_err()
            .code,
        "SOURCE_CONTEXT_MISMATCH"
    );
}

#[test]
fn returned_query_and_account_boundary_are_verified() {
    let row = account(12, "1");
    for field in ["query", "sourceSlot"] {
        let mut capture = Capture::new(&[row.clone()], identity("capture", false));
        let mut changed = row.clone();
        if field == "query" {
            changed["query"]["slot"] = json!(13);
        } else {
            changed["value"][field] = json!(11);
        }
        capture.replace_record(&serde_json::to_vec(&changed).expect("record"));
        assert_eq!(
            capture
                .open()
                .expect("manifest")
                .inspect(&row["query"])
                .expect_err("wrong boundary")
                .code,
            "SOURCE_CONTEXT_MISMATCH"
        );
    }
}

#[test]
fn invalid_account_shape_numbers_and_evidence_fail_closed() {
    for (field, value) in [
        ("evidenceHashes", json!([])),
        ("evidenceHashes", json!(["bad"])),
        ("lamports", json!(9007199254740992_u64)),
        ("lamports", json!("18446744073709551616")),
        ("rentEpoch", json!("01")),
        ("dataBase64", json!("YQ")),
        ("owner", json!("bad")),
        ("role", json!("unknown")),
    ] {
        let mut row = account(12, "1");
        if field == "evidenceHashes" {
            row[field] = value;
        } else {
            row["value"][field] = value;
        }
        let capture = Capture::new(&[row.clone()], identity("capture", false));
        assert_eq!(
            capture
                .open()
                .expect("manifest")
                .inspect(&row["query"])
                .expect_err("invalid record")
                .code,
            "SOURCE_INTEGRITY",
            "{field}"
        );
    }
}

#[test]
fn duplicate_queries_mixed_genesis_and_undeclared_capability_reject() {
    let row = account(12, "1");
    assert_eq!(
        Capture::new(&[row.clone(), row.clone()], identity("capture", false))
            .open()
            .err()
            .expect("duplicate query")
            .code,
        "INVALID_SOURCE"
    );
    let mut wrong = identity("capture", false);
    wrong["genesisHash"] = json!(key(9));
    assert_eq!(
        Capture::new(&[row.clone()], wrong)
            .open()
            .err()
            .expect("wrong genesis")
            .code,
        "SOURCE_CONTEXT_MISMATCH"
    );
    let mut missing = identity("capture", false);
    missing["capabilities"] = json!(["block"]);
    assert_eq!(
        Capture::new(&[row], missing)
            .open()
            .err()
            .expect("undeclared capability")
            .code,
        "INVALID_SOURCE"
    );
}

#[test]
fn duplicate_json_keys_reject_with_valid_file_digests() {
    let row = account(12, "1");
    let mut capture = Capture::new(&[row.clone()], identity("capture", false));
    let bytes = serde_json::to_string(&row).expect("record");
    let duplicate = bytes.replacen('{', "{\"value\":null,", 1);
    capture.replace_record(duplicate.as_bytes());
    assert_eq!(
        capture
            .open()
            .expect("manifest")
            .inspect(&row["query"])
            .expect_err("duplicate record field")
            .code,
        "SOURCE_INTEGRITY"
    );
    let manifest = serde_json::to_string(&capture.manifest)
        .expect("manifest")
        .replacen('{', "{\"schema\":\"wrong\",", 1);
    std::fs::write(capture.path(), &manifest).expect("duplicate manifest fixture");
    assert_eq!(
        CapturedSource::open(&capture.path(), &Digest::of(&manifest))
            .err()
            .expect("duplicate manifest field")
            .code,
        "SOURCE_INTEGRITY"
    );
}

#[test]
fn block_digest_binds_original_bytes_and_complete_envelope() {
    let raw = format!(
        "{{ \"result\": {{\"parentSlot\":10,\"blockhash\":\"{}\",\"transactions\":[]}} }}",
        key(3)
    );
    let row = json!({"query":{"kind":"block","genesisHash":key(1),"slot":12,"blockEvidenceSha256":hash(&raw)},
        "value":{"rawBase64":STANDARD.encode(&raw)},"evidenceHashes":[hash(&raw)]});
    let capture = Capture::new(&[row.clone()], identity("capture", false));
    assert!(capture
        .open()
        .expect("open")
        .inspect(&row["query"])
        .expect("inspect")
        .is_some());
    let mut changed = row.clone();
    changed["value"]["rawBase64"] = json!(STANDARD.encode(
        serde_json::to_vec(&serde_json::from_str::<Value>(&raw).expect("raw fixture"))
            .expect("reencode")
    ));
    assert_eq!(
        Capture::new(&[changed], identity("capture", false))
            .open()
            .expect("open")
            .inspect(&row["query"])
            .expect_err("raw bytes changed")
            .code,
        "SOURCE_INTEGRITY"
    );
    for raw in [
        format!(
            "{{\"result\":{{\"parentSlot\":12,\"blockhash\":\"{}\",\"transactions\":[]}}}}",
            key(3)
        ),
        "{\"result\":null}".into(),
        "{\"result\":{},\"error\":null}".into(),
        "{\"result\":{},\"result\":{}}".into(),
    ] {
        let mut bad = row.clone();
        bad["query"]["blockEvidenceSha256"] = json!(hash(&raw));
        bad["value"]["rawBase64"] = json!(STANDARD.encode(raw));
        assert!(Capture::new(&[bad.clone()], identity("capture", false))
            .open()
            .expect("manifest")
            .inspect(&bad["query"])
            .is_err());
    }
}
