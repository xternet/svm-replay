use super::*;

#[test]
#[ignore = "offline operator helper: freezes only the reviewed16 missing queries"]
fn freeze_reviewed_rent_queries() {
    let result = (|| {
        let (summary, pin) = read_pinned(
            "SVM_REPLAY_TEST_RENT_SUMMARY",
            "SVM_REPLAY_TEST_RENT_SUMMARY_SHA256",
        )?;
        let frozen = freeze(&summary, &pin)?;
        let path = new_output("SVM_REPLAY_TEST_RENT_QUERIES")?;
        let digest = write_new(&path, &frozen)?;
        println!(
            "{}",
            json!({"queries":path,"sha256":digest,"count":16,"networkRequests":0})
        );
        Ok::<_, SourceError>(())
    })();
    result.expect("freeze exact Rent queries");
}

#[test]
#[ignore = "LIVE operator helper: explicit approval environment required; never run in ordinary test sweeps"]
fn capture_missing_target_rent() {
    let result = (|| {
        require(
            env("SVM_REPLAY_TEST_RENT_CAPTURE_ENABLE")? == "approved-exact-rent-16",
            "explicit live-capture enable required",
        )?;
        let (frozen, pin) = read_pinned(
            "SVM_REPLAY_TEST_RENT_QUERIES",
            "SVM_REPLAY_TEST_RENT_QUERIES_SHA256",
        )?;
        let queries = validate_frozen(&frozen)?;
        let directory = new_output("SVM_REPLAY_TEST_RENT_CAPTURE_DIR")?;
        let receipt = capture(&directory, &queries, &pin, || {
            AlchemySource::from_environment(config(&queries), limits())
        })?;
        println!("{}", json!({"directory":directory,"receipt":receipt}));
        require(
            receipt["status"] == "COMPLETE",
            "capture stopped; inspect typed receipt, do not retry or bypass authorization errors",
        )
    })();
    result.expect("bounded exact Rent capture");
}

#[test]
fn freeze_derives_unique_missing_queries_and_rejects_scope_expansion() {
    let baseline = synthetic_summary();
    let frozen = freeze(&baseline, &Digest::of(b"synthetic-summary-only")).unwrap();
    assert_eq!(validate_frozen(&frozen).unwrap().len(), 16);
    for mode in 0..7 {
        let mut bad = baseline.clone();
        let q = &mut bad["results"][30]["error"]["details"]["sourceError"]["details"]["query"];
        match mode {
            0 => q["pubkey"] = json!("SysvarC1ock11111111111111111111111111111111"),
            1 => q["genesisHash"] = json!("11111111111111111111111111111111"),
            2 => q["phase"] = json!("current"),
            3 => q["slot"] = json!(-1),
            4 => q["minContextSlot"] = json!(100),
            5 => bad["results"][30]["error"]["code"] = json!("SOURCE_HTTP_ERROR"),
            6 => {
                bad["results"].as_array_mut().unwrap().pop();
            }
            _ => unreachable!(),
        }
        assert!(
            freeze(&bad, &Digest::of(b"synthetic-summary-only")).is_err(),
            "mode {mode}"
        );
    }
    let mut duplicate = frozen;
    duplicate["queries"][1] = duplicate["queries"][0].clone();
    assert!(validate_frozen(&duplicate).is_err());
}

#[test]
fn offline_capture_is_exact_provenance_bound_and_stops_at_sixteen_reads() {
    let frozen = freeze(&synthetic_summary(), &Digest::of(b"synthetic-only")).unwrap();
    let queries = validate_frozen(&frozen).unwrap();
    let transport = Arc::new(SyntheticTransport {
        calls: Mutex::new(vec![]),
        failure: None,
    });
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("capture");
    let receipt = capture(&directory, &queries, &Digest::of(b"frozen"), || {
        AlchemySource::with_transport(config(&queries), limits(), transport.clone())
    })
    .unwrap();
    assert_eq!(receipt["status"], "COMPLETE");
    assert_eq!(receipt["counters"]["requests"], 17);
    assert_eq!(receipt["counters"]["account_reads"], 16);
    assert_eq!(transport.calls.lock().unwrap().len(), 17);
    let pin = Digest::new(receipt["manifestSha256"].as_str().unwrap()).unwrap();
    let captured = CapturedSource::open(&directory.join("manifest.json"), &pin).unwrap();
    for q in &queries {
        let record = captured.inspect(q).unwrap().unwrap();
        assert_eq!(record["value"]["rentEpoch"], u64::MAX.to_string());
        assert_eq!(record["value"]["lamports"], "9007199254740993");
    }
    assert!(
        capture(&directory, &queries, &Digest::of(b"frozen"), || panic!(
            "existing path must reject before source construction"
        ))
        .is_err()
    );
}

#[test]
fn offline_http_and_rpc_errors_persist_partial_counters_without_retry_or_body() {
    for status in [401, 403, 429, 500, 200] {
        let queries =
            validate_frozen(&freeze(&synthetic_summary(), &Digest::of(b"synthetic")).unwrap())
                .unwrap();
        let transport = Arc::new(SyntheticTransport {
            calls: Mutex::new(vec![]),
            failure: Some(status),
        });
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("capture");
        let receipt = capture(&directory, &queries, &Digest::of(b"frozen"), || {
            AlchemySource::with_transport(config(&queries), limits(), transport.clone())
        })
        .unwrap();
        assert_eq!(receipt["status"], "STOPPED_ERROR");
        assert_eq!(receipt["counters"]["requests"], 2);
        assert_eq!(receipt["counters"]["account_reads"], 1);
        assert_eq!(transport.calls.lock().unwrap().len(), 2);
        if status == 200 {
            assert_eq!(receipt["error"]["code"], "SOURCE_RPC_ERROR");
            assert_eq!(receipt["error"]["details"]["rpcCode"], -32005);
        } else {
            assert_eq!(receipt["error"]["details"]["httpStatus"], status);
        }
        assert_eq!(receipt["recordsCaptured"], 0);
        let bytes = std::fs::read(directory.join("capture-receipt.json")).unwrap();
        assert!(!String::from_utf8(bytes)
            .unwrap()
            .contains("sensitive-provider-error"));
        assert!(directory.join("manifest.json").is_file());
    }
}

#[test]
fn offline_configuration_failure_persists_zero_counters_without_factory_retry() {
    let queries =
        validate_frozen(&freeze(&synthetic_summary(), &Digest::of(b"synthetic")).unwrap()).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let receipt = capture(
        &temporary.path().join("capture"),
        &queries,
        &Digest::of(b"frozen"),
        || {
            Err(SourceError::new(
                "SOURCE_CONFIGURATION",
                "synthetic missing configuration",
            ))
        },
    )
    .unwrap();
    assert_eq!(receipt["status"], "STOPPED_ERROR");
    assert_eq!(receipt["error"]["code"], "SOURCE_CONFIGURATION");
    assert_eq!(receipt["counters"]["requests"], 0);
    assert_eq!(receipt["counters"]["account_reads"], 0);
}
