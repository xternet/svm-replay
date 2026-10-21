use super::*;

fn block(parent: u64) -> Value {
    json!({"jsonrpc":"2.0","id":1,"result":{"parentSlot":parent,"blockhash":key(4),
        "previousBlockhash":key(3),"blockHeight":80,"blockTime":123,"transactions":[],"rewards":[]}})
}

#[test]
fn next_produced_block_uses_finalized_discovery_and_parent_link_including_skipped_slots() {
    for after in [94, 95] {
        let transport = Transport::values(vec![
            genesis(),
            json!({"jsonrpc":"2.0","id":1,"result":[96]}),
            block(94),
        ]);
        let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
        let record = source.discover_next_block(after).unwrap();
        assert_eq!(record["query"]["slot"], 96);
        assert_eq!(
            transport.requests.lock().unwrap()[1],
            json!({"jsonrpc":"2.0","id":1,"method":"getBlocksWithLimit","params":[after+1,1,{"commitment":"finalized"}]})
        );
        assert_eq!(source.counters().unwrap().requests, 3);
    }
}

#[test]
fn next_block_rejects_missing_unsafe_out_of_coverage_or_nonadjacent_results() {
    for slots in [
        json!([]),
        json!([94]),
        json!([101]),
        json!([96, 97]),
        json!([u64::MAX]),
    ] {
        let transport = Transport::values(vec![
            genesis(),
            json!({"jsonrpc":"2.0","id":1,"result":slots}),
        ]);
        let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
        assert!(source.discover_next_block(94).is_err());
        assert!(transport.requests.lock().unwrap().len() <= 2);
    }
    let transport = Transport::values(vec![
        genesis(),
        json!({"jsonrpc":"2.0","id":1,"result":[99]}),
        block(98),
    ]);
    let source = AlchemySource::with_transport(config(), limits(), transport).unwrap();
    assert_eq!(
        source.discover_next_block(94).unwrap_err().code,
        "SOURCE_CONTEXT_MISMATCH"
    );
    let transport = Transport::values(vec![]);
    let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
    assert!(source.discover_next_block(100).is_err());
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn next_block_discovery_is_fully_cached_without_new_provider_calls() {
    let transport = Transport::values(vec![
        genesis(),
        json!({"jsonrpc":"2.0","id":1,"result":[96]}),
        block(94),
    ]);
    let root = tempfile::tempdir().unwrap();
    let first = AlchemySource::with_transport(config(), limits(), transport.clone())
        .unwrap()
        .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    let record = first.discover_next_block(94).unwrap();
    let second = AlchemySource::with_transport(config(), limits(), transport.clone())
        .unwrap()
        .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    assert_eq!(second.discover_next_block(94).unwrap(), record);
    assert_eq!(second.counters().unwrap().requests, 0);
    assert_eq!(transport.requests.lock().unwrap().len(), 3);
}
