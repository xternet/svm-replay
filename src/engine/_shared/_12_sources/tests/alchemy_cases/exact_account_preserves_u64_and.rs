use super::*;

#[test]
fn exact_account_preserves_u64_and_redacted_request_response_provenance() {
    let raw = account();
    let transport = Transport::values(vec![genesis(), raw.clone()]);
    let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
    let record = source.inspect(&query(2)).unwrap().unwrap();
    assert_eq!(record["value"]["lamports"], "9007199254740993");
    assert_eq!(record["value"]["rentEpoch"], u64::MAX.to_string());
    assert_eq!(record["value"]["sourceSlot"], 99);
    assert_eq!(record["value"]["presence"], "present");
    let requests = transport.requests.lock().unwrap();
    assert_eq!(
        requests[0],
        json!({"jsonrpc":"2.0","id":1,"method":"getGenesisHash","params":[]})
    );
    assert_eq!(
        requests[1],
        json!([{"jsonrpc":"2.0","id":0,"method":"getAccountInfo","params":[key(2),{"commitment":"finalized","encoding":"base64","slot":99}]}])
    );
    assert_eq!(record["provenance"]["request"], requests[1]);
    assert_eq!(
        record["provenance"]["responseSha256"],
        json!(Digest::of(serde_json::to_vec(&raw).unwrap()))
    );
    assert_eq!(
        record["provenance"]["genesisResponseSha256"],
        json!(Digest::of(serde_json::to_vec(&genesis()).unwrap()))
    );
    let counters = source.counters().unwrap();
    assert_eq!(counters.requests, 2);
    assert_eq!(counters.account_reads, 1);
    assert!(counters.downloaded_bytes > 0);
    assert_eq!(source.identity()["coverage"]["completeness"], "partial");
    let diagnostics = source.diagnostics().unwrap();
    assert_eq!(diagnostics["status"], "REPORTED");
    assert_eq!(diagnostics["transport"]["requests"], 2);
    assert_eq!(diagnostics["transport"]["account_reads"], 1);
    assert_eq!(
        diagnostics["transport"]["methods"],
        json!({
            "getGenesisHash": 1, "getAccountInfo": 1
        })
    );
    assert_eq!(
        diagnostics["transport"]["downloaded_bytes"],
        serde_json::to_vec(&genesis()).unwrap().len() + serde_json::to_vec(&raw).unwrap().len()
    );
    assert!(diagnostics["durableBudget"].is_null());
}

#[test]
fn discovered_block_is_pinned_and_retains_provider_provenance() {
    let block = json!({"jsonrpc":"2.0","id":1,"result":{"parentSlot":98,
        "blockhash":key(2),"previousBlockhash":key(3),"transactions":[]}});
    let transport = Transport::values(vec![genesis(), block.clone()]);
    let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
    let record = source.discover_block(99).unwrap();
    let expected = Digest::of(serde_json::to_vec(&block).unwrap());
    assert_eq!(record["query"]["blockEvidenceSha256"], json!(expected));
    assert_eq!(record["provenance"]["responseSha256"], json!(expected));
    assert_eq!(source.counters().unwrap().requests, 2);
    assert_eq!(
        source.discover_block(101).unwrap_err().code,
        "SOURCE_UNAVAILABLE"
    );
    assert_eq!(source.counters().unwrap().requests, 2);
}

#[test]
fn composite_discovery_uses_provider_cache_without_a_second_inspect_fetch() {
    use svm_replay_engine::shared::sources::cached::{
        source_identity_sha256, CacheScope, CachedSource,
    };
    let root = tempfile::tempdir().unwrap();
    let raw_cache = tempfile::tempdir().unwrap();
    let block = json!({"jsonrpc":"2.0","id":1,"result":{"parentSlot":98,
        "blockhash":key(2),"previousBlockhash":key(3),"transactions":[]}});
    let transport = Transport::values(vec![genesis(), block]);
    let source = AlchemySource::with_transport(config(), limits(), transport.clone())
        .unwrap()
        .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    let scope = CacheScope {
        namespace_id: "discovery-test".into(),
        expected_source_identity_sha256: source_identity_sha256(&source.identity()).unwrap(),
        pin_owner: None,
    };
    let cached = CachedSource::new(
        Arc::new(source),
        Arc::new(Mutex::new(
            svm_replay_store::Store::open(raw_cache.path()).unwrap(),
        )),
        scope,
    )
    .unwrap();
    let mut composite = CompositeSource::new(vec![Box::new(cached)]).unwrap();
    let first = composite.discover_block(99).unwrap();
    assert_eq!(composite.discover_block(99).unwrap(), first);
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    assert_eq!(
        first["observations"][0]["provenance"]["schema"],
        "m17-alchemy-rpc-provenance/v1"
    );
    let artifacts = tempfile::tempdir().unwrap();
    let mut composite = composite
        .with_discovery_artifacts(artifacts.path())
        .unwrap();
    let spooled = composite.discover_block(99).unwrap();
    let observation = &spooled["observations"][0];
    assert!(observation.get("provenance").is_none());
    let reference = &observation["provenanceArtifact"];
    let bytes = std::fs::read(artifacts.path().join(reference["file"].as_str().unwrap())).unwrap();
    assert_eq!(reference["sha256"], json!(Digest::of(&bytes)));
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        first["observations"][0]["provenance"]
    );
    assert_eq!(composite.discover_block(99).unwrap(), spooled);
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    std::fs::write(
        artifacts.path().join(reference["file"].as_str().unwrap()),
        b"corrupt",
    )
    .unwrap();
    assert_eq!(
        composite.discover_block(99).unwrap_err().code,
        "SOURCE_INTEGRITY"
    );
}

#[test]
fn block_discovery_rejects_incomplete_data_wrong_network_and_budget_exhaustion() {
    for block in [
        json!({"jsonrpc":"2.0","id":1,"result":null}),
        json!({"jsonrpc":"2.0","id":1,"result":{"parentSlot":98}}),
        json!({"jsonrpc":"2.0","id":1,"result":{"parentSlot":99,"transactions":[]}}),
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"private upstream error"}}),
    ] {
        let source = AlchemySource::with_transport(
            config(),
            limits(),
            Transport::values(vec![genesis(), block]),
        )
        .unwrap();
        let error = source.discover_block(99).unwrap_err();
        assert!(!error.to_string().contains("private upstream error"));
        assert_eq!(source.counters().unwrap().failed_inspections, 1);
    }
    let source = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![json!({"jsonrpc":"2.0","id":1,"result":key(9)})]),
    )
    .unwrap();
    assert_eq!(
        source.discover_block(99).unwrap_err().code,
        "SOURCE_CONTEXT_MISMATCH"
    );
    assert_eq!(source.counters().unwrap().requests, 1);
    let mut bounds = limits();
    bounds.max_requests = 1;
    let source =
        AlchemySource::with_transport(config(), bounds, Transport::values(vec![genesis()]))
            .unwrap();
    assert_eq!(
        source.discover_block(99).unwrap_err().code,
        "SOURCE_RESOURCE_LIMIT"
    );
    assert_eq!(source.counters().unwrap().requests, 1);
}

#[test]
fn diagnostics_retain_failed_transport_counts_and_separate_durable_reservations() {
    let dir = tempfile::tempdir().unwrap();
    let bounded = limits();
    let budget = Arc::new(
        DurableRpcBudget::open(
            svm_replay_store::Store::open(dir.path()).unwrap(),
            "diagnostics-test".into(),
            &bounded,
        )
        .unwrap(),
    );
    let transport = Transport::values(vec![genesis()]);
    transport
        .responses
        .lock()
        .unwrap()
        .push_back(Err(TransportFailure::Network));
    let source = AlchemySource::with_transport(config(), bounded, transport)
        .unwrap()
        .with_budget(budget)
        .unwrap();
    assert_eq!(
        source.inspect(&query(2)).unwrap_err().code,
        "SOURCE_TRANSPORT"
    );
    let diagnostics = source.diagnostics().unwrap();
    assert_eq!(diagnostics["transport"]["requests"], 2);
    assert_eq!(diagnostics["transport"]["account_reads"], 1);
    assert_eq!(diagnostics["transport"]["failed_inspections"], 1);
    assert_eq!(
        diagnostics["transport"]["downloaded_bytes"],
        serde_json::to_vec(&genesis()).unwrap().len()
    );
    assert_eq!(diagnostics["durableBudget"]["reserved"]["requests"], 2);
    assert_eq!(
        diagnostics["durableBudget"]["reserved"]["bytes"],
        2 * (limits().max_response_bytes + 1)
    );
}

#[test]
fn explicit_null_is_absence_but_wrong_context_or_undefined_value_is_error() {
    let mut absent = account();
    absent[0]["result"]["value"] = Value::Null;
    let t = Transport::values(vec![genesis(), absent]);
    let s = AlchemySource::with_transport(config(), limits(), t).unwrap();
    assert_eq!(
        s.inspect(&query(2)).unwrap().unwrap()["value"],
        json!({"pubkey":key(2),"sourceSlot":99,"role":"application","presence":"absent"})
    );
    for mode in 0..5 {
        let mut bad = account();
        match mode {
            0 => bad[0]["result"]["context"]["slot"] = json!(100),
            1 => {
                bad[0]["result"].as_object_mut().unwrap().remove("value");
            }
            2 => bad[0]["id"] = json!(1),
            3 => {
                let copy = bad[0].clone();
                bad.as_array_mut().unwrap().push(copy);
            }
            4 => bad[0]["jsonrpc"] = json!("1.0"),
            _ => unreachable!(),
        };
        let s = AlchemySource::with_transport(
            config(),
            limits(),
            Transport::values(vec![genesis(), bad]),
        )
        .unwrap();
        assert!(s.inspect(&query(2)).is_err(), "mode {mode}");
    }
}
