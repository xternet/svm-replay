use super::*;

#[test]
fn bad_query_phase_network_numbers_unknown_fields_and_public_ids_reject() {
    let source = SuppliedBankSource::new(identity("bank", true), vec![bank()]).expect("supplied");
    for (field, value) in [
        ("genesisHash", json!(key(9))),
        ("phase", json!("end-slot")),
        ("slot", json!(9007199254740992_u64)),
        ("fallback", json!("current")),
        ("parentSlot", json!(12)),
        ("input", json!("unknown")),
    ] {
        let mut query = bank()["query"].clone();
        query[field] = value;
        assert!(source.inspect(&query).is_err(), "{field}");
    }
    let mut id = identity("bank", true);
    id["id"] = json!("https://secret.example/key");
    assert!(SuppliedBankSource::new(id, vec![bank()]).is_err());
    assert!(SuppliedBankSource::new(identity("bank", true), vec![bank(), bank()]).is_err());
    assert!(CompositeSource::new(vec![]).is_err());
}

#[test]
fn query_key_matches_m11_domain_and_sorted_coordinates() {
    let query = account(12, "1")["query"].clone();
    let expected=format!("m11-source-query/v1\n{{\"genesisHash\":\"{}\",\"kind\":\"account\",\"phase\":\"end-slot\",\"pubkey\":\"{}\",\"slot\":12}}\n",key(1),key(2));
    assert_eq!(query_key(&query).expect("key"), Digest::of(expected));
    let original = bank()["query"].clone();
    let mut fork = original.clone();
    fork["blockEvidenceSha256"] = json!(hash("other block"));
    assert_ne!(
        query_key(&original).expect("key"),
        query_key(&fork).expect("fork key")
    );
    assert_ne!(
        query_key(&query).expect("key"),
        query_key(&account(13, "1")["query"]).expect("other slot")
    );
}

#[test]
fn local_io_error_keeps_path_and_errno_instead_of_unavailable_history() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("absent.json");
    let error = CapturedSource::open(&path, &Digest::of("missing"))
        .err()
        .expect("missing capture");
    assert_eq!(error.code, "SOURCE_IO_ERROR");
    assert_eq!(
        error.details.expect("details"),
        json!({"path":path,"errno":"ENOENT"})
    );
    let row = account(12, "1");
    let capture = Capture::new(&[row.clone()], identity("capture", false));
    let source = capture.open().expect("open");
    std::fs::remove_file(capture.dir.path().join("record-0.json"))
        .expect("remove only test record");
    assert_eq!(
        source
            .inspect(&row["query"])
            .expect_err("missing manifested record")
            .code,
        "SOURCE_IO_ERROR"
    );
}

#[test]
fn composite_rejects_duplicate_identity_and_mixed_networks_before_reads() {
    let same = || SuppliedBankSource::new(identity("bank", true), vec![bank()]).expect("source");
    assert_eq!(
        CompositeSource::new(vec![Box::new(same()), Box::new(same())])
            .err()
            .expect("duplicate identity")
            .code,
        "INVALID_SOURCE"
    );
    let mut other_identity = identity("bank", true);
    other_identity["genesisHash"] = json!(key(9));
    let other = SuppliedBankSource::new(other_identity, vec![]).expect("empty explicit source");
    assert_eq!(
        CompositeSource::new(vec![Box::new(same()), Box::new(other)])
            .err()
            .expect("mixed network takes precedence")
            .code,
        "SOURCE_CONTEXT_MISMATCH"
    );
}

#[test]
fn unavailable_details_contain_only_current_query_observations() {
    let capture = Capture::new(&[account(12, "1")], identity("capture", false));
    let mut source =
        CompositeSource::new(vec![Box::new(capture.open().expect("capture"))]).expect("composite");
    source
        .require(&account(12, "1")["query"])
        .expect("available");
    let error = source
        .require(&account(13, "1")["query"])
        .expect_err("missing");
    let details = error.details.expect("details");
    assert_eq!(
        details["observations"]
            .as_array()
            .expect("observations")
            .len(),
        1
    );
    assert_eq!(details["observations"][0]["query"]["slot"], 13);
    assert_eq!(details["observations"][0]["status"], "UNAVAILABLE");
    assert_eq!(source.observations().len(), 2);
}
