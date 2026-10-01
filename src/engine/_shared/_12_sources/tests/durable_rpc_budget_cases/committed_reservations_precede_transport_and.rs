use super::*;

#[test]
fn committed_reservations_precede_transport_and_success_keeps_full_response_bound() {
    let dir = tempfile::tempdir().unwrap();
    let limits = limits();
    let ledger = budget(dir.path(), &limits);
    let transport = Transport::new(dir.path(), vec![genesis(), account()]);
    let source = source(limits, transport.clone(), ledger.clone());
    source.inspect(&query(2)).unwrap().unwrap();
    assert_eq!(
        transport.observed.lock().unwrap()[0]["used"],
        json!({"reads":0,"requests":1,"bytes":1025})
    );
    assert_eq!(
        transport.observed.lock().unwrap()[1]["used"],
        json!({"reads":1,"requests":2,"bytes":2050})
    );
    let snapshot = ledger.snapshot().unwrap();
    assert_eq!(
        snapshot["accounting"],
        "full-response-reservation/no-refund"
    );
    assert_eq!(
        snapshot["reserved"],
        json!({"reads":1,"requests":2,"bytes":2050})
    );
    let counters = source.counters().unwrap();
    assert_eq!(counters.charged_bytes, 2050);
    assert!(counters.downloaded_bytes > 0 && counters.downloaded_bytes < 2050);
}

#[test]
fn remaining_durable_allowance_bounds_the_last_response_and_unavailable_reads_are_free() {
    let dir = tempfile::tempdir().unwrap();
    let limits = SourceLimits {
        max_download_bytes: 1500,
        ..limits()
    };
    let ledger = budget(dir.path(), &limits);
    let transport = Transport::new(dir.path(), vec![genesis(), account()]);
    let source = source(limits, transport.clone(), ledger.clone());
    let mut unavailable = query(2);
    unavailable["slot"] = json!(101);
    assert!(source.inspect(&unavailable).unwrap().is_none());
    assert_eq!(
        ledger.snapshot().unwrap()["reserved"],
        json!({"reads":0,"requests":0,"bytes":0})
    );
    source.inspect(&query(2)).unwrap().unwrap();
    assert_eq!(transport.observed.lock().unwrap()[1]["transportBound"], 474);
    assert_eq!(ledger.snapshot().unwrap()["reserved"]["bytes"], 1500);
    assert_eq!(source.counters().unwrap().charged_bytes, 1500);
}

#[test]
fn last_byte_cannot_start_a_request_without_payload_and_lookahead_reservation() {
    let dir = tempfile::tempdir().unwrap();
    let limits = SourceLimits {
        max_download_bytes: 1,
        ..limits()
    };
    let ledger = budget(dir.path(), &limits);
    let transport = Transport::new(dir.path(), vec![]);
    let source = source(limits, transport.clone(), ledger.clone());
    assert_eq!(
        source.inspect(&query(2)).unwrap_err().code,
        "SOURCE_RESOURCE_LIMIT"
    );
    assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        ledger.snapshot().unwrap()["reserved"],
        json!({"reads":0,"requests":0,"bytes":0})
    );
}

#[test]
fn restart_keeps_spend_exhaustion_blocks_network_and_limits_cannot_change() {
    let dir = tempfile::tempdir().unwrap();
    let limits = SourceLimits {
        max_download_bytes: 2048,
        ..limits()
    };
    let ledger = budget(dir.path(), &limits);
    let transport = Transport::new(dir.path(), vec![genesis(), account()]);
    let first = source(limits.clone(), transport, ledger.clone());
    first.inspect(&query(2)).unwrap();
    drop(first);
    drop(ledger);
    let reopened = budget(dir.path(), &limits);
    assert_eq!(reopened.snapshot().unwrap()["reserved"]["bytes"], 2048);
    let no_network = Transport::new(dir.path(), vec![]);
    let second = source(limits.clone(), no_network.clone(), reopened);
    assert_eq!(
        second.inspect(&query(3)).unwrap_err().code,
        "SOURCE_RESOURCE_LIMIT"
    );
    assert_eq!(no_network.calls.load(Ordering::SeqCst), 0);
    assert_eq!(second.counters().unwrap().requests, 0);
    let changed = SourceLimits {
        max_download_bytes: 4096,
        ..limits
    };
    let error = DurableRpcBudget::open(
        Store::open(dir.path()).unwrap(),
        "campaign-1".into(),
        &changed,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "SOURCE_BUDGET_CONFLICT");
    assert_eq!(inspect(dir.path(), "campaign-1")["used"]["bytes"], 2048);
}

#[test]
fn unknown_partial_failure_and_http_error_keep_full_spend_after_reopen() {
    for failed in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let limits = limits();
        let failure = if failed {
            Err(TransportFailure::Network)
        } else {
            Ok(RpcHttpResponse {
                status: 429,
                body: b"provider failure".to_vec(),
            })
        };
        let transport = Transport::new(dir.path(), vec![genesis(), failure]);
        let first = source(limits.clone(), transport, budget(dir.path(), &limits));
        let error = first.inspect(&query(2)).unwrap_err();
        assert_eq!(
            error.code,
            if failed {
                "SOURCE_TRANSPORT"
            } else {
                "SOURCE_RATE_LIMIT"
            }
        );
        assert_eq!(first.counters().unwrap().charged_bytes, 2050);
        drop(first);
        assert_eq!(
            budget(dir.path(), &limits).snapshot().unwrap()["reserved"],
            json!({"reads":1,"requests":2,"bytes":2050})
        );
    }
}

#[test]
fn untrusted_transport_overshoot_is_persisted_and_never_refunded() {
    let dir = tempfile::tempdir().unwrap();
    let limits = SourceLimits {
        max_download_bytes: 1024,
        ..limits()
    };
    let transport = Transport::new(
        dir.path(),
        vec![Ok(RpcHttpResponse {
            status: 200,
            body: vec![b'x'; 1500],
        })],
    );
    let first = source(limits.clone(), transport, budget(dir.path(), &limits));
    assert_eq!(
        first.inspect(&query(2)).unwrap_err().code,
        "SOURCE_RESOURCE_LIMIT"
    );
    drop(first);
    assert_eq!(
        budget(dir.path(), &limits).snapshot().unwrap()["reserved"],
        json!({"reads":0,"requests":1,"bytes":1500})
    );
}
