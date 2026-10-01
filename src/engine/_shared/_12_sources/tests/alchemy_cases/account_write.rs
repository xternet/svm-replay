use super::*;

fn write_query() -> Value {
    let mut q = query(2);
    q["phase"] = json!("last-write-at-or-before-slot");
    q
}

#[test]
fn archived_write_retains_actual_slot_and_cannot_alias_end_slot() {
    for write_slot in [90, 99] {
        let mut raw = account();
        raw[0]["result"]["context"]["slot"] = json!(write_slot);
        let transport = Transport::values(vec![genesis(), raw]);
        let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
        let q = write_query();
        let record = source.inspect(&q).unwrap().unwrap();
        assert_eq!(record["value"]["sourceSlot"], write_slot);
        assert_eq!(record["query"]["slot"], 99);
        assert_eq!(record["query"]["phase"], "last-write-at-or-before-slot");
        assert_eq!(record["value"]["lamports"], "9007199254740993");
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests[1][0]["params"][1]["lastUpdateBeforeSlot"], 100);
        assert!(requests[1][0]["params"][1].get("slot").is_none());
        assert_eq!(source.counters().unwrap().account_reads, 1);
        assert_ne!(
            svm_replay_engine::shared::sources::query_key(&q).unwrap(),
            svm_replay_engine::shared::sources::query_key(&query(2)).unwrap()
        );
    }
}

#[test]
fn archived_write_rejects_future_context_and_preserves_deletion_and_errors() {
    let mut future = account();
    future[0]["result"]["context"]["slot"] = json!(100);
    let s = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![genesis(), future]),
    )
    .unwrap();
    assert_eq!(
        s.inspect(&write_query()).unwrap_err().code,
        "SOURCE_CONTEXT_MISMATCH"
    );
    let mut deleted = account();
    deleted[0]["result"]["context"]["slot"] = json!(98);
    deleted[0]["result"]["value"] = Value::Null;
    let s = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![genesis(), deleted]),
    )
    .unwrap();
    let record = s.inspect(&write_query()).unwrap().unwrap();
    assert_eq!(record["value"]["sourceSlot"], 98);
    assert_eq!(record["value"]["presence"], "absent");
    let failure = json!([{"jsonrpc":"2.0","id":0,"error":{"code":-32020,"message":"excluded"}}]);
    let s = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![genesis(), failure]),
    )
    .unwrap();
    assert_eq!(
        s.inspect(&write_query()).unwrap_err().code,
        "SOURCE_RPC_ERROR"
    );
}

#[test]
fn write_cursor_overflow_is_rejected_before_any_rpc() {
    let transport = Transport::values(vec![]);
    let s = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
    let mut q = write_query();
    q["slot"] = json!(9_007_199_254_740_991u64);
    assert_eq!(s.inspect(&q).unwrap_err().code, "INVALID_QUERY");
    assert!(transport.requests.lock().unwrap().is_empty());
}
