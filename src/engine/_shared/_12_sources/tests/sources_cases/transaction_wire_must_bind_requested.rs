use super::*;

#[test]
fn transaction_wire_must_bind_requested_signature() {
    let mut bytes = vec![1];
    bytes.extend([7; 64]);
    bytes.extend([1, 0, 1, 2]);
    bytes.extend([1; 32]);
    bytes.extend([2; 32]);
    bytes.extend([3; 32]);
    bytes.extend([1, 1, 1, 0, 2, 4, 5]);
    let row = json!({"query":{"kind":"transaction","genesisHash":key(1),"slot":12,"signature":bs58::encode([7;64]).into_string()},"value":STANDARD.encode(&bytes),"evidenceHashes":[hash(&bytes)]});
    assert!(Capture::new(&[row.clone()], identity("capture", false))
        .open()
        .expect("manifest")
        .inspect(&row["query"])
        .expect("transaction")
        .is_some());
    bytes[1] = 8;
    let mut changed = row.clone();
    changed["value"] = json!(STANDARD.encode(bytes));
    assert_eq!(
        Capture::new(&[changed], identity("capture", false))
            .open()
            .expect("manifest")
            .inspect(&row["query"])
            .expect_err("different signature")
            .code,
        "SOURCE_CONTEXT_MISMATCH"
    );
    let mut invalid = row.clone();
    invalid["value"] = json!(STANDARD.encode([0]));
    assert_eq!(
        Capture::new(&[invalid], identity("capture", false))
            .open()
            .expect("manifest")
            .inspect(&row["query"])
            .expect_err("invalid wire")
            .code,
        "SOURCE_INTEGRITY"
    );
}

#[test]
fn captured_and_supplied_contexts_compose_without_relabeling() {
    let row = bank();
    let capture = Capture::new(&[account(12, "1")], identity("capture", false));
    let bank_source =
        SuppliedBankSource::new(identity("bank", true), vec![row.clone()]).expect("supplied");
    let mut source = CompositeSource::new(vec![
        Box::new(capture.open().expect("capture")),
        Box::new(bank_source),
    ])
    .expect("composite");
    assert_eq!(
        source.require(&row["query"]).expect("bank")["value"],
        row["value"]
    );
    let result = source.require(&account(12, "1")["query"]).expect("account");
    assert_eq!(result["observations"][1]["status"], "UNAVAILABLE");
    let kinds: Vec<_> = source
        .observations()
        .iter()
        .filter(|o| o["status"] == "AVAILABLE")
        .map(|o| o["source"]["kind"].clone())
        .collect();
    assert_eq!(
        kinds,
        vec![json!("supplied-bank"), json!("captured-history")]
    );
    assert!(SuppliedBankSource::new(identity("bank", true), vec![account(12, "1")]).is_err());
}

#[test]
fn supplied_inputs_outputs_and_identities_are_owned_snapshots() {
    let mut row = bank();
    let mut id = identity("bank", true);
    let expected = row.clone();
    let source = SuppliedBankSource::new(id.clone(), vec![row.clone()]).expect("supplied");
    row["value"]["snapshot"]["complete"] = json!(false);
    id["coverage"]["lastSlot"] = json!(0);
    let mut first = source
        .inspect(&expected["query"])
        .expect("inspect")
        .expect("record");
    first["value"]["snapshot"]["complete"] = json!(false);
    let mut returned_id = source.identity();
    returned_id["coverage"]["lastSlot"] = json!(0);
    let actual = source
        .inspect(&expected["query"])
        .expect("inspect")
        .expect("record");
    assert_eq!(actual["value"], expected["value"]);
    assert_eq!(
        actual["evidenceHashes"].as_array().expect("hashes").len(),
        2
    );
}

#[test]
fn agreeing_sources_keep_all_evidence_but_role_is_not_chain_state() {
    let a = account(12, "1");
    let mut b = a.clone();
    b["value"]["role"] = json!("sysvar");
    b["evidenceHashes"] = json!([hash("other evidence")]);
    let one = Capture::new(&[a.clone()], identity("one", false));
    let two = Capture::new(&[b.clone()], identity("two", false));
    let mut source = CompositeSource::new(vec![
        Box::new(one.open().expect("one")),
        Box::new(two.open().expect("two")),
    ])
    .expect("composite");
    let result = source.read(&a["query"]).expect("agree");
    assert_eq!(
        result["observations"][0]["valueSha256"],
        result["observations"][1]["valueSha256"]
    );
    let evidence = result["evidenceHashes"].as_array().expect("hashes");
    assert!(evidence.contains(&a["evidenceHashes"][0]));
    assert!(evidence.contains(&b["evidenceHashes"][0]));
}

#[test]
fn conflicts_do_not_prefer_first_source() {
    let one = Capture::new(&[account(12, "1")], identity("one", false));
    let two = Capture::new(&[account(12, "4")], identity("two", false));
    let mut source = CompositeSource::new(vec![
        Box::new(one.open().expect("one")),
        Box::new(two.open().expect("two")),
    ])
    .expect("composite");
    let error = source
        .read(&account(12, "1")["query"])
        .expect_err("conflict");
    assert_eq!(error.code, "SOURCE_CONFLICT");
    assert_eq!(
        error.details.expect("details")["sourceIds"],
        json!(["one", "two"])
    );
}

#[test]
fn invalid_evidence_aborts_before_next_source() {
    let mut bad = account(12, "1");
    bad["evidenceHashes"] = json!(["bad"]);
    let calls = Arc::new(AtomicUsize::new(0));
    let mut source = CompositeSource::new(vec![
        controlled("bad", bad, Arc::new(AtomicUsize::new(0)), false),
        controlled("good", account(12, "1"), calls.clone(), false),
    ])
    .expect("composite");
    assert_eq!(
        source
            .require(&account(12, "1")["query"])
            .expect_err("invalid evidence")
            .code,
        "SOURCE_INTEGRITY"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn source_identity_mutation_during_read_is_an_error() {
    let mut source = CompositeSource::new(vec![controlled(
        "mutable",
        account(12, "1"),
        Arc::new(AtomicUsize::new(0)),
        true,
    )])
    .expect("composite");
    assert_eq!(
        source
            .require(&account(12, "1")["query"])
            .expect_err("mutated identity")
            .code,
        "SOURCE_INTEGRITY"
    );
}
