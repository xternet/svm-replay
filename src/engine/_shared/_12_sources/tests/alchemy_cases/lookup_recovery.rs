use super::*;

fn signature() -> String {
    bs58::encode([2u8; 64]).into_string()
}
fn status(value: Value) -> Value {
    json!({"jsonrpc":"2.0","id":1,"result":{"context":{"slot":100},"value":[value]}})
}
fn wire() -> String {
    let mut bytes = vec![1];
    bytes.extend([2u8; 64]);
    bytes.extend([1, 0, 0, 1]);
    bytes.extend([3u8; 32]);
    bytes.extend([4u8; 32]);
    bytes.push(0);
    STANDARD.encode(bytes)
}
fn block(transactions: Value) -> Value {
    json!({"jsonrpc":"2.0","id":1,"result":{"parentSlot":98,"transactions":transactions}})
}

#[test]
fn finalized_status_recovers_missing_transaction_location() {
    let transport = Transport::values(vec![
        genesis(),
        json!({"jsonrpc":"2.0","id":1,"result":null}),
        status(json!({"slot":99,"confirmationStatus":"finalized","err":null})),
    ]);
    let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
    let value = source.discover_transaction(&signature()).unwrap();
    assert_eq!(value["slot"], 99);
    assert_eq!(value["lookupKind"], "signature-status");
    assert_eq!(
        transport.requests.lock().unwrap()[2]["params"],
        json!([[signature()],{"searchTransactionHistory":true}])
    );
}

#[test]
fn historical_block_recovers_exact_wire_with_revalidated_provenance() {
    let transport = Transport::values(vec![
        genesis(),
        json!([{"jsonrpc":"2.0","id":0,"result":null}]),
        block(json!([{"transaction":[wire(),"base64"]}])),
    ]);
    let source = AlchemySource::with_transport(config(), limits(), transport).unwrap();
    let query =
        json!({"kind":"transaction","genesisHash":key(1),"slot":99,"signature":signature()});
    let record = source.inspect(&query).unwrap().unwrap();
    assert_eq!(record["value"], wire());
    assert_eq!(record["provenance"]["request"]["method"], "getBlock");
}

#[test]
fn lookup_recovery_rejects_nonfinalized_missing_and_outside_coverage() {
    for value in [
        Value::Null,
        json!({"slot":99,"confirmationStatus":"confirmed"}),
        json!({"slot":101,"confirmationStatus":"finalized"}),
    ] {
        let transport = Transport::values(vec![
            genesis(),
            json!({"jsonrpc":"2.0","id":1,"result":null}),
            status(value),
        ]);
        let source = AlchemySource::with_transport(config(), limits(), transport).unwrap();
        assert!(source.discover_transaction(&signature()).is_err());
    }
}

#[test]
fn block_wire_recovery_rejects_missing_duplicate_and_malformed_wire() {
    for txs in [
        json!([]),
        json!([{"transaction":[wire(),"base64"]},{"transaction":[wire(),"base64"]}]),
        json!([{"transaction":["AA==","base64"]}]),
    ] {
        let transport = Transport::values(vec![
            genesis(),
            json!([{"jsonrpc":"2.0","id":0,"result":null}]),
            block(txs),
        ]);
        let source = AlchemySource::with_transport(config(), limits(), transport).unwrap();
        let query =
            json!({"kind":"transaction","genesisHash":key(1),"slot":99,"signature":signature()});
        assert!(source.inspect(&query).is_err());
    }
}

#[test]
fn recovered_location_survives_restart_without_network() {
    let root = tempfile::tempdir().unwrap();
    let transport = Transport::values(vec![
        genesis(),
        json!({"jsonrpc":"2.0","id":1,"result":null}),
        status(json!({"slot":99,"confirmationStatus":"finalized"})),
    ]);
    let source = AlchemySource::with_transport(config(), limits(), transport)
        .unwrap()
        .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    let result = source.discover_transaction(&signature()).unwrap();
    drop(source);
    let empty = Transport::values(vec![]);
    let warm = AlchemySource::with_transport(config(), limits(), empty.clone())
        .unwrap()
        .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    assert_eq!(warm.discover_transaction(&signature()).unwrap(), result);
    assert!(empty.requests.lock().unwrap().is_empty());
}

#[test]
fn rpc_errors_never_trigger_alternative_lookups() {
    let transport = Transport::values(vec![
        genesis(),
        json!({"jsonrpc":"2.0","id":1,"error":{"code":429}}),
    ]);
    let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
    assert_eq!(
        source.discover_transaction(&signature()).unwrap_err().code,
        "SOURCE_RATE_LIMIT"
    );
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}
