use super::*;

#[test]
fn duplicate_safe_raw_json_and_complete_block_hash_binding() {
    let t = Transport::values(vec![genesis()]);
    t.responses.lock().unwrap().push_back(Ok(RpcHttpResponse{status:200,body:br#"[{"jsonrpc":"2.0","id":0,"result":{"context":{"slot":99},"value":null,"value":null}}]"#.to_vec()}));
    let s = AlchemySource::with_transport(config(), limits(), t).unwrap();
    assert_eq!(s.inspect(&query(2)).unwrap_err().code, "SOURCE_INTEGRITY");
    let block = json!({"jsonrpc":"2.0","id":1,"result":{"parentSlot":98,"blockhash":key(5),"transactions":[]}});
    let bytes = serde_json::to_vec(&block).unwrap();
    let q = json!({"kind":"block","genesisHash":key(1),"slot":99,"blockEvidenceSha256":Digest::of(&bytes)});
    let s = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![genesis(), block.clone()]),
    )
    .unwrap();
    let record = s.inspect(&q).unwrap().unwrap();
    assert_eq!(
        STANDARD
            .decode(record["value"]["rawBase64"].as_str().unwrap())
            .unwrap(),
        bytes
    );
    let mut wrong = q.clone();
    wrong["blockEvidenceSha256"] = json!(Digest::of(b"wrong"));
    let s = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![genesis(), block]),
    )
    .unwrap();
    assert_eq!(s.inspect(&wrong).unwrap_err().code, "SOURCE_INTEGRITY");
}

#[test]
fn composite_keeps_live_provenance_in_its_observations() {
    let s = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![genesis(), account()]),
    )
    .unwrap();
    let mut composite = CompositeSource::new(vec![Box::new(s)]).unwrap();
    let value = composite.require(&query(2)).unwrap();
    assert_eq!(
        value["observations"][0]["source"]["kind"],
        "alchemy-historical"
    );
    assert!(value["observations"][0]["provenance"].is_object());
    let diagnostics = composite.diagnostics().unwrap();
    assert_eq!(diagnostics["scope"], "source-instance-cumulative");
    assert_eq!(
        diagnostics["sources"][0]["diagnostics"]["transport"]["requests"],
        2
    );
}

#[test]
fn transaction_wire_is_bound_to_exact_finalized_slot_signature_and_encoding() {
    let mut bytes = vec![1];
    bytes.extend([7; 64]);
    bytes.extend([1, 0, 1, 2]);
    bytes.extend([1; 32]);
    bytes.extend([2; 32]);
    bytes.extend([3; 32]);
    bytes.extend([1, 1, 1, 0, 2, 4, 5]);
    let wire = STANDARD.encode(&bytes);
    let q = json!({"kind":"transaction","genesisHash":key(1),"slot":99,"signature":bs58::encode([7;64]).into_string()});
    for mode in 0..5 {
        let mut result =
            json!([{"jsonrpc":"2.0","id":0,"result":{"slot":99,"transaction":[wire,"base64"]}}]);
        match mode {
            0 => {}
            1 => result[0]["result"]["slot"] = json!(100),
            2 => result[0]["result"]["transaction"][1] = json!("json"),
            3 => result[0]["result"]["transaction"][0] = json!("AQID"),
            4 => {
                let mut changed = bytes.clone();
                changed[1] = 8;
                result[0]["result"]["transaction"][0] = json!(STANDARD.encode(changed));
            }
            _ => unreachable!(),
        };
        let t = Transport::values(vec![genesis(), result]);
        let s = AlchemySource::with_transport(config(), limits(), t.clone()).unwrap();
        let output = s.inspect(&q);
        if mode == 0 {
            assert_eq!(output.unwrap().unwrap()["value"], wire);
            assert_eq!(
                t.requests.lock().unwrap()[1][0]["params"][1]["commitment"],
                "finalized"
            );
        } else {
            assert!(output.is_err(), "mode {mode}");
        }
    }
}

#[test]
fn cached_live_records_keep_raw_bytes_and_reject_derived_value_or_provenance_tampering() {
    use svm_replay_engine::shared::sources::cached::{
        raw_cache_key, source_identity_sha256, CacheScope, CachedSource,
    };
    use svm_replay_store::Store;
    for tamper in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
        let t = Transport::values(vec![genesis(), account()]);
        let source =
            Arc::new(AlchemySource::with_transport(config(), limits(), t.clone()).unwrap());
        let scope = CacheScope {
            namespace_id: "case".into(),
            expected_source_identity_sha256: source_identity_sha256(&source.identity()).unwrap(),
            pin_owner: None,
        };
        if tamper == 0 {
            let cache = CachedSource::new(source.clone(), store, scope).unwrap();
            let a = cache.inspect(&query(2)).unwrap().unwrap();
            let b = cache.inspect(&query(2)).unwrap().unwrap();
            assert_eq!(a, b);
            assert_eq!(
                STANDARD
                    .decode(a["provenance"]["responseBodyBase64"].as_str().unwrap())
                    .unwrap(),
                serde_json::to_vec(&account()).unwrap()
            );
            assert_eq!(t.requests.lock().unwrap().len(), 2);
            t.responses.lock().unwrap().push_back(Ok(RpcHttpResponse {
                status: 200,
                body: serde_json::to_vec(&account()).unwrap(),
            }));
            cache.inspect(&query(3)).unwrap();
            assert_eq!(t.requests.lock().unwrap().len(), 3);
            let diagnostics = cache.diagnostics().unwrap();
            assert_eq!(diagnostics["cache"]["hits"], 1);
            assert_eq!(diagnostics["cache"]["misses"], 2);
            assert_eq!(diagnostics["underlying"]["transport"]["requests"], 3);
            assert_eq!(diagnostics["underlying"]["transport"]["account_reads"], 2);
            assert_eq!(
                diagnostics["underlying"]["transport"]["downloaded_bytes"],
                serde_json::to_vec(&genesis()).unwrap().len()
                    + 2 * serde_json::to_vec(&account()).unwrap().len()
            );
            cache.inspect(&query(2)).unwrap();
            cache.inspect(&query(3)).unwrap();
            assert_eq!(t.requests.lock().unwrap().len(), 3);
            assert_eq!(cache.diagnostics().unwrap()["cache"]["hits"], 3);
            assert_eq!(
                cache.diagnostics().unwrap()["underlying"],
                diagnostics["underlying"]
            );
        } else {
            let mut record = source.inspect(&query(2)).unwrap().unwrap();
            match tamper {
                1 => record["value"]["lamports"] = json!("12"),
                2 => record["provenance"]["request"][0]["params"][1]["slot"] = json!(100),
                3 => {
                    record["provenance"]["responseBodyBase64"] =
                        json!(STANDARD.encode(b"different bytes"))
                }
                _ => unreachable!(),
            };
            let cache_key = raw_cache_key(&scope, &source.identity(), &query(2)).unwrap();
            let entry = json!({"schema":"m17-raw-source-cache/v1","namespace":"case","source":source.identity(),"query":query(2),"record":record});
            store.lock().unwrap().execute(json!({"version":1,"op":"put","namespace":"raw","key":cache_key,"dataBase64":STANDARD.encode(serde_json::to_vec(&entry).unwrap())})).unwrap();
            let cache = CachedSource::new(source, store, scope).unwrap();
            assert_eq!(
                cache.inspect(&query(2)).unwrap_err().code,
                "SOURCE_INTEGRITY"
            );
            assert_eq!(t.requests.lock().unwrap().len(), 2);
        }
    }
}
