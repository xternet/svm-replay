use super::*;

#[test]
fn cache_fill_uses_distinct_handle_and_warm_restart_has_no_ledger_or_rpc_charge() {
    let dir = tempfile::tempdir().unwrap();
    let limits = SourceLimits {
        max_download_bytes: 2048,
        ..limits()
    };
    let first = Arc::new(source(
        limits.clone(),
        Transport::new(dir.path(), vec![genesis(), account()]),
        budget(dir.path(), &limits),
    ));
    let scope = CacheScope {
        namespace_id: "variants".into(),
        expected_source_identity_sha256: source_identity_sha256(&first.identity()).unwrap(),
        pin_owner: None,
    };
    let cache = CachedSource::new(
        first.clone(),
        Arc::new(Mutex::new(Store::open(dir.path()).unwrap())),
        scope.clone(),
    )
    .unwrap();
    let expected = cache.inspect(&query(2)).unwrap().unwrap();
    assert_eq!(cache.inspect(&query(2)).unwrap().unwrap(), expected);
    let before = inspect(dir.path(), "campaign-1");
    drop(cache);
    drop(first);
    let no_network = Transport::new(dir.path(), vec![]);
    let second = Arc::new(source(
        limits.clone(),
        no_network.clone(),
        budget(dir.path(), &limits),
    ));
    let warm = CachedSource::new(
        second.clone(),
        Arc::new(Mutex::new(Store::open(dir.path()).unwrap())),
        scope,
    )
    .unwrap();
    assert_eq!(warm.inspect(&query(2)).unwrap().unwrap(), expected);
    assert_eq!(inspect(dir.path(), "campaign-1"), before);
    assert_eq!(no_network.calls.load(Ordering::SeqCst), 0);
    assert_eq!(second.counters().unwrap().requests, 0);
}

#[test]
fn independent_ledger_handles_atomically_share_id_across_provider_identities() {
    let dir = tempfile::tempdir().unwrap();
    let limits = SourceLimits {
        max_requests: 1,
        ..limits()
    };
    let start = Arc::new(Barrier::new(2));
    let mut threads = vec![];
    let mut transports = vec![];
    for provider in ["provider-a", "provider-b"] {
        let transport = Transport::new(dir.path(), vec![genesis()]);
        transports.push(transport.clone());
        let source = AlchemySource::with_transport(
            AlchemyConfig {
                id: provider.into(),
                ..config()
            },
            limits.clone(),
            transport,
        )
        .unwrap()
        .with_budget(budget(dir.path(), &limits))
        .unwrap();
        let start = start.clone();
        threads.push(std::thread::spawn(move || {
            start.wait();
            source.inspect(&query(2)).unwrap_err().code
        }));
    }
    for thread in threads {
        assert_eq!(thread.join().unwrap(), "SOURCE_RESOURCE_LIMIT");
    }
    assert_eq!(
        transports
            .iter()
            .map(|transport| transport.calls.load(Ordering::SeqCst))
            .sum::<usize>(),
        1
    );
    assert_eq!(
        inspect(dir.path(), "campaign-1")["used"],
        json!({"reads":0,"requests":1,"bytes":1025})
    );
}

#[test]
fn attachment_requires_unused_source_matching_limits_and_safe_explicit_id() {
    let dir = tempfile::tempdir().unwrap();
    let limits = limits();
    let ledger = budget(dir.path(), &limits);
    let plain = AlchemySource::with_transport(
        config(),
        limits.clone(),
        Transport::new(dir.path(), vec![genesis(), account()]),
    )
    .unwrap();
    // This transport checks a committed ledger; reserve beforehand solely for the
    // unrelated plain-source use needed to test the attach-after-use rejection.
    Store::open(dir.path()).unwrap().execute(json!({"version":1,"op":"budget","action":"charge","id":"campaign-1","amount":{"reads":1,"requests":2,"bytes":2048}})).unwrap();
    plain.inspect(&query(2)).unwrap();
    assert_eq!(
        plain.with_budget(ledger.clone()).err().unwrap().code,
        "SOURCE_CONFIGURATION"
    );
    let incompatible = SourceLimits {
        max_requests: 9,
        ..limits.clone()
    };
    let incompatible_source =
        AlchemySource::with_transport(config(), incompatible, Transport::new(dir.path(), vec![]))
            .unwrap();
    assert_eq!(
        incompatible_source
            .with_budget(ledger.clone())
            .err()
            .unwrap()
            .code,
        "SOURCE_CONFIGURATION"
    );
    let source = source(
        limits.clone(),
        Transport::new(dir.path(), vec![]),
        ledger.clone(),
    );
    assert_eq!(
        source.with_budget(ledger).err().unwrap().code,
        "SOURCE_CONFIGURATION"
    );
    for id in ["", "https://secret", "secret?key=value"] {
        assert_eq!(
            DurableRpcBudget::open(Store::open(dir.path()).unwrap(), id.into(), &limits)
                .err()
                .unwrap()
                .code,
            "SOURCE_CONFIGURATION"
        );
    }
}

#[test]
#[ignore = "helper invoked in a subprocess by the SIGKILL reservation test"]
fn reservation_crash_child() {
    struct KillAfterReservation(PathBuf);
    impl RpcTransport for KillAfterReservation {
        fn post(
            &self,
            _: &[u8],
            _: Duration,
            bound: u64,
        ) -> Result<RpcHttpResponse, TransportFailure> {
            assert_eq!(inspect(&self.0, "campaign-1")["used"]["bytes"], bound + 1);
            #[cfg(unix)]
            let status = std::process::Command::new("/bin/kill")
                .args(["-KILL", &std::process::id().to_string()])
                .status()
                .unwrap();
            #[cfg(windows)]
            let status = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &std::process::id().to_string()])
                .status()
                .unwrap();
            panic!("test force-kill unexpectedly returned: {status}");
        }
    }
    let root = PathBuf::from(
        std::env::var_os("SVM_REPLAY_RPC_BUDGET_CRASH_ROOT").expect("test child root"),
    );
    let limits = SourceLimits {
        max_download_bytes: 1024,
        ..limits()
    };
    let source = source(
        limits.clone(),
        Arc::new(KillAfterReservation(root.clone())),
        budget(&root, &limits),
    );
    source.inspect(&query(2)).unwrap();
    panic!("crash child unexpectedly completed");
}
