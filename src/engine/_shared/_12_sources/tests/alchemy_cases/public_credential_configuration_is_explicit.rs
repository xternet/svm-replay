use super::*;

#[test]
fn public_credential_configuration_is_explicit_and_offline() {
    const CHILD: &str = "SVM_REPLAY_CREDENTIAL_TEST_CHILD";
    if let Ok(mode) = std::env::var(CHILD) {
        let result = AlchemySource::from_environment(config(), limits());
        if mode == "valid" {
            assert!(result.is_ok());
        } else {
            let error = result
                .err()
                .expect("invalid or absent public key must reject");
            assert_eq!(error.code, "SOURCE_CONFIGURATION");
            if mode == "missing" {
                assert!(error.message.contains("API_ALCHEMY"));
            }
        }
        return;
    }
    for (mode, key) in [
        ("missing", None),
        ("empty", Some("")),
        ("invalid", Some("https://not-a-key")),
        ("valid", Some("test-only-sentinel")),
    ] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "public_credential_configuration_is_explicit::public_credential_configuration_is_explicit_and_offline",
                "--nocapture",
            ])
            .env_remove("API_ALCHEMY")
            .env("API_ALCHEMY_SOLANA_HIST", "legacy-test-sentinel")
            .env(CHILD, mode);
        if let Some(key) = key {
            child.env("API_ALCHEMY", key);
        }
        let output = child.output().expect("credential test subprocess");
        assert!(
            output.status.success(),
            "{mode}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn finalized_discovery_survives_restart_without_rpc() {
    let root = tempfile::tempdir().unwrap();
    let sig = "1".repeat(64);
    let lookup =
        json!({"jsonrpc":"2.0","id":1,"result":{"slot":99,"transaction":{"signatures":[sig]}}});
    let transport = Transport::values(vec![genesis(), lookup]);
    let source = AlchemySource::with_transport(config(), limits(), transport.clone())
        .unwrap()
        .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    let found = source.discover_transaction(&sig).unwrap();
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    drop(source);
    let no_requests = Transport::values(vec![]);
    let warm = AlchemySource::with_transport(config(), limits(), no_requests.clone())
        .unwrap()
        .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    assert_eq!(warm.discover_transaction(&sig).unwrap(), found);
    assert!(no_requests.requests.lock().unwrap().is_empty());
    assert_eq!(warm.counters().unwrap().requests, 0);
}

#[test]
fn cancelled_source_never_sends_a_request() {
    let transport = Transport::values(vec![]);
    let token = svm_replay_engine::shared::runtime::CancellationToken::default();
    token.cancel();
    let source = AlchemySource::with_transport(config(), limits(), transport.clone())
        .unwrap()
        .with_cancellation(token);
    assert_eq!(source.inspect(&query(2)).unwrap_err().code, "CANCELLED");
    assert!(transport.requests.lock().unwrap().is_empty());
    assert_eq!(source.counters().unwrap().requests, 0);
}

#[test]
fn finalized_block_cache_is_reusable_and_provider_bound() {
    let root = tempfile::tempdir().unwrap();
    let block = json!({"jsonrpc":"2.0","id":1,"result":{"parentSlot":98,
        "blockhash":key(2),"previousBlockhash":key(3),"transactions":[]}});
    let cold = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![genesis(), block.clone()]),
    )
    .unwrap()
    .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    let found = cold.discover_block(99).unwrap();
    drop(cold);
    let empty = Transport::values(vec![]);
    let warm = AlchemySource::with_transport(config(), limits(), empty.clone())
        .unwrap()
        .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    assert_eq!(warm.discover_block(99).unwrap(), found);
    assert!(empty.requests.lock().unwrap().is_empty());
    drop(warm);
    let mut changed = config();
    changed.version = "different".into();
    let transport = Transport::values(vec![genesis(), block]);
    let other = AlchemySource::with_transport(changed, limits(), transport.clone())
        .unwrap()
        .with_discovery_cache(svm_replay_store::Store::open(root.path()).unwrap());
    other.discover_block(99).unwrap();
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[test]
fn invalid_discovery_cache_is_rejected_without_refetch() {
    let root = tempfile::tempdir().unwrap();
    let sig = "1".repeat(64);
    let lookup =
        json!({"jsonrpc":"2.0","id":1,"result":{"slot":99,"transaction":{"signatures":[sig]}}});
    let source = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![genesis(), lookup]),
    )
    .unwrap();
    let mut found = source.discover_transaction(&sig).unwrap();
    found["slot"] = json!(98);
    let identity = json!({"schema":"svm-finalized-discovery/v1","source":source.identity(),
        "query":{"kind":"signature","signature":sig}});
    let cache_key = format!(
        "discovery/{}",
        Digest::of(serde_json::to_vec(&identity).unwrap()).as_str()
    );
    let mut store = svm_replay_store::Store::open(root.path()).unwrap();
    let entry = json!({"identity":identity,"value":found});
    store
        .execute(
            json!({"version":1,"op":"put","namespace":"raw","key":cache_key,
        "dataBase64":STANDARD.encode(serde_json::to_vec(&entry).unwrap())}),
        )
        .unwrap();
    let empty = Transport::values(vec![]);
    let cached = AlchemySource::with_transport(config(), limits(), empty.clone())
        .unwrap()
        .with_discovery_cache(store);
    assert_eq!(
        cached.discover_transaction(&sig).unwrap_err().code,
        "SOURCE_INTEGRITY"
    );
    assert!(empty.requests.lock().unwrap().is_empty());
}
