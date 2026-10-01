use super::*;

#[test]
fn account_data_integer_encoding_and_owner_are_validated_without_rounding() {
    for patch in [
        json!({"lamports":"01"}),
        json!({"lamports":"18446744073709551616"}),
        json!({"lamports":-1}),
        json!({"lamports":1.5}),
        json!({"rentEpoch":"1e3"}),
        json!({"executable":null}),
        json!({"owner":"bad"}),
        json!({"data":["AB==","base64"]}),
        json!({"data":["AQID","base58"]}),
    ] {
        let mut raw = account();
        for (k, v) in patch.as_object().unwrap() {
            raw[0]["result"]["value"][k] = v.clone();
        }
        let s = AlchemySource::with_transport(
            config(),
            limits(),
            Transport::values(vec![genesis(), raw]),
        )
        .unwrap();
        assert_eq!(s.inspect(&query(2)).unwrap_err().code, "SOURCE_INTEGRITY");
    }
    let mut raw = account();
    raw[0]["result"]["value"]["lamports"] = json!("18446744073709551615");
    let s =
        AlchemySource::with_transport(config(), limits(), Transport::values(vec![genesis(), raw]))
            .unwrap();
    assert_eq!(
        s.inspect(&query(2)).unwrap().unwrap()["value"]["lamports"],
        u64::MAX.to_string()
    );
}

#[test]
fn network_mismatch_unsupported_bank_and_outside_coverage_never_fall_back() {
    let t = Transport::values(vec![]);
    let s = AlchemySource::with_transport(config(), limits(), t.clone()).unwrap();
    let mut q = query(2);
    q["slot"] = json!(101);
    assert!(s.inspect(&q).unwrap().is_none());
    q["genesisHash"] = json!(key(9));
    assert_eq!(s.inspect(&q).unwrap_err().code, "SOURCE_CONTEXT_MISMATCH");
    let bank = json!({"kind":"bank-input","genesisHash":key(1),"slot":99,"parentSlot":98,"blockhash":key(4),"blockEvidenceSha256":Digest::of(b"block"),"executorSourceId":"reviewed","runtimeProfileId":"profile","activeFeatureSetHash":Digest::of(b"features"),"phase":"post-bank-initialization/pre-transaction","input":"initializationEvidence"});
    assert!(s.inspect(&bank).unwrap().is_none());
    assert!(t.requests.lock().unwrap().is_empty());
    let mut wrong = genesis();
    wrong["result"] = json!(key(9));
    let t = Transport::values(vec![wrong]);
    let s = AlchemySource::with_transport(config(), limits(), t.clone()).unwrap();
    assert_eq!(
        s.inspect(&query(2)).unwrap_err().code,
        "SOURCE_CONTEXT_MISMATCH"
    );
    assert_eq!(t.requests.lock().unwrap().len(), 1);
}

#[test]
fn http_rpc_rate_and_transport_errors_stay_explicit_and_do_not_leak_body() {
    for status in [401, 402, 403, 429, 500] {
        let t = Transport::values(vec![genesis()]);
        t.responses.lock().unwrap().push_back(Ok(RpcHttpResponse {
            status,
            body: b"endpoint/credential-secret".to_vec(),
        }));
        let s = AlchemySource::with_transport(config(), limits(), t).unwrap();
        let e = s.inspect(&query(2)).unwrap_err();
        assert_eq!(
            e.code,
            if status == 429 {
                "SOURCE_RATE_LIMIT"
            } else {
                "SOURCE_HTTP_ERROR"
            }
        );
        assert!(!e.to_string().contains("credential-secret"));
        let hint = match status {
            401 => "API_ALCHEMY",
            402 => "billing",
            403 => "Archive",
            429 => "credits",
            _ => "HTTP",
        };
        assert!(e.to_string().contains(hint), "missing actionable hint: {e}");
    }
    let rpc = json!([{"jsonrpc":"2.0","id":0,"error":{"code":-32005,"message":"credential-secret","data":"credential-secret"}}]);
    let s =
        AlchemySource::with_transport(config(), limits(), Transport::values(vec![genesis(), rpc]))
            .unwrap();
    let e = s.inspect(&query(2)).unwrap_err();
    assert_eq!(e.code, "SOURCE_RPC_ERROR");
    assert!(!serde_json::to_string(&e)
        .unwrap()
        .contains("credential-secret"));
    let t = Transport::values(vec![]);
    t.responses
        .lock()
        .unwrap()
        .push_back(Err(TransportFailure::Timeout));
    let s = AlchemySource::with_transport(config(), limits(), t).unwrap();
    assert_eq!(s.inspect(&query(2)).unwrap_err().code, "SOURCE_DEADLINE");
}

#[test]
fn request_read_response_and_total_byte_limits_are_enforced() {
    for budget in [
        SourceLimits {
            max_requests: 1,
            ..limits()
        },
        SourceLimits {
            max_account_reads: 1,
            ..limits()
        },
    ] {
        let s = AlchemySource::with_transport(
            config(),
            budget,
            Transport::values(vec![genesis(), account()]),
        )
        .unwrap();
        if s.inspect(&query(2)).is_ok() {
            assert_eq!(
                s.inspect(&query(3)).unwrap_err().code,
                "SOURCE_RESOURCE_LIMIT"
            );
        } else {
            assert_eq!(s.counters().unwrap().requests, 1);
        }
    }
    let g = serde_json::to_vec(&genesis()).unwrap().len() as u64;
    for budget in [
        SourceLimits {
            max_response_bytes: g,
            ..limits()
        },
        SourceLimits {
            max_download_bytes: g + 1,
            ..limits()
        },
    ] {
        let s = AlchemySource::with_transport(
            config(),
            budget,
            Transport::values(vec![genesis(), account()]),
        )
        .unwrap();
        assert_eq!(
            s.inspect(&query(2)).unwrap_err().code,
            "SOURCE_RESOURCE_LIMIT"
        );
    }
    assert!(AlchemySource::with_transport(
        config(),
        SourceLimits {
            deadline: Duration::ZERO,
            ..limits()
        },
        Transport::values(vec![])
    )
    .is_err());
}

#[test]
fn failed_transport_reserves_its_full_byte_allowance_instead_of_refunding_unknown_reads() {
    let t = Transport::values(vec![]);
    t.responses
        .lock()
        .unwrap()
        .push_back(Err(TransportFailure::Network));
    let s = AlchemySource::with_transport(
        config(),
        SourceLimits {
            max_download_bytes: 100,
            max_response_bytes: 100,
            ..limits()
        },
        t.clone(),
    )
    .unwrap();
    assert_eq!(s.inspect(&query(2)).unwrap_err().code, "SOURCE_TRANSPORT");
    assert_eq!(
        s.inspect(&query(2)).unwrap_err().code,
        "SOURCE_RESOURCE_LIMIT"
    );
    assert_eq!(t.requests.lock().unwrap().len(), 1);
    let counters = s.counters().unwrap();
    assert_eq!(counters.downloaded_bytes, 0);
    assert_eq!(counters.charged_bytes, 100);
}
