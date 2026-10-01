use super::*;

#[test]
fn repeated_variant_reuses_exact_absence_and_expanded_dependency_fetches_only_missing() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
    let source = Source::new();
    let cache = CachedSource::new(source.clone(), store.clone(), scope("run-a", false)).unwrap();
    let original = cache.inspect(&query(2)).unwrap().unwrap();
    assert_eq!(original, record(&query(2)));
    assert_eq!(cache.inspect(&query(2)).unwrap().unwrap(), original);
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    cache.inspect(&query(3)).unwrap();
    assert_eq!(source.reads.load(Ordering::SeqCst), 2);
    let diagnostics = cache.diagnostics().unwrap();
    assert_eq!(diagnostics["cache"]["inspectCalls"], 3);
    assert_eq!(diagnostics["cache"]["hits"], 1);
    assert_eq!(diagnostics["cache"]["misses"], 2);
    assert_eq!(diagnostics["cache"]["fills"], 2);
    assert_eq!(diagnostics["cache"]["failedInspections"], 0);
    assert_eq!(diagnostics["underlying"]["status"], "NOT_REPORTED");
    assert!(diagnostics["underlying"].get("transport").is_none());
    let reopened = CachedSource::new(
        source.clone(),
        Arc::new(Mutex::new(Store::open(dir.path()).unwrap())),
        scope("run-a", false),
    )
    .unwrap();
    assert_eq!(reopened.inspect(&query(2)).unwrap().unwrap(), original);
    assert_eq!(source.reads.load(Ordering::SeqCst), 2);
    let isolated = CachedSource::new(source.clone(), store, scope("run-b", false)).unwrap();
    isolated.inspect(&query(2)).unwrap();
    assert_eq!(source.reads.load(Ordering::SeqCst), 3);
}

#[test]
fn unavailable_rate_errors_and_invalid_observations_are_never_negative_cached() {
    for mode in [1, 2, 3] {
        let dir = tempfile::tempdir().unwrap();
        let source = Source::new();
        *source.mode.lock().unwrap() = mode;
        let cache = CachedSource::new(
            source.clone(),
            Arc::new(Mutex::new(Store::open(dir.path()).unwrap())),
            scope("run", false),
        )
        .unwrap();
        for _ in 0..2 {
            let output = cache.inspect(&query(2));
            if mode == 1 {
                assert!(output.unwrap().is_none());
            } else {
                assert!(output.is_err());
            }
        }
        assert_eq!(source.reads.load(Ordering::SeqCst), 2);
        let diagnostics = cache.diagnostics().unwrap();
        assert_eq!(diagnostics["cache"]["misses"], 2);
        assert_eq!(diagnostics["cache"]["fills"], 0);
        assert_eq!(
            diagnostics["cache"]["failedInspections"],
            if mode == 1 { 0 } else { 2 }
        );
        *source.mode.lock().unwrap() = 0;
        cache.inspect(&query(2)).unwrap();
        assert_eq!(source.reads.load(Ordering::SeqCst), 3);
    }
}

#[test]
fn caller_identity_pin_source_mutation_and_complete_query_binding_are_strict() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
    let source = Source::new();
    let bad = CacheScope {
        expected_source_identity_sha256: Digest::of(b"wrong identity"),
        ..scope("run", false)
    };
    assert!(CachedSource::new(source.clone(), store.clone(), bad).is_err());
    let cache = CachedSource::new(source.clone(), store, scope("run", false)).unwrap();
    cache.inspect(&query(2)).unwrap();
    source.identity.lock().unwrap()["version"] = json!("2");
    assert_eq!(
        cache.inspect(&query(2)).unwrap_err().code,
        "SOURCE_INTEGRITY"
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    let a = raw_cache_key(&scope("run", false), &identity(), &query(2)).unwrap();
    let mut changed = query(2);
    changed["slot"] = json!(98);
    assert_ne!(
        a,
        raw_cache_key(&scope("run", false), &identity(), &changed).unwrap()
    );
    let mut provider = identity();
    provider["version"] = json!("2");
    let new_scope = CacheScope {
        expected_source_identity_sha256: source_identity_sha256(&provider).unwrap(),
        ..scope("run", false)
    };
    assert_ne!(a, raw_cache_key(&new_scope, &provider, &query(2)).unwrap());
}

#[test]
fn corrupt_cache_record_never_triggers_provider_fallback_and_pins_are_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
    let source = Source::new();
    let cache = CachedSource::new(source.clone(), store.clone(), scope("pinned", true)).unwrap();
    cache.inspect(&query(2)).unwrap();
    let eviction = store
        .lock()
        .unwrap()
        .execute(json!({"version":1,"op":"evict","maxBytes":0}))
        .unwrap();
    assert_eq!(eviction["limitSatisfied"], false);
    assert_eq!(
        cache.inspect(&query(2)).unwrap().unwrap(),
        record(&query(2))
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    let other = scope("corrupt", false);
    let key = raw_cache_key(&other, &identity(), &query(2)).unwrap();
    store.lock().unwrap().execute(json!({"version":1,"op":"put","namespace":"raw","key":key,"dataBase64":STANDARD.encode(b"{\"bad\":true}")})).unwrap();
    let cache = CachedSource::new(source.clone(), store, other).unwrap();
    assert_eq!(
        cache.inspect(&query(2)).unwrap_err().code,
        "SOURCE_INTEGRITY"
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
}

#[test]
fn one_shared_store_handle_coalesces_parallel_positive_reads_without_unsafe_sync() {
    let dir = tempfile::tempdir().unwrap();
    let source = Source::new();
    let cache = Arc::new(
        CachedSource::new(
            source.clone(),
            Arc::new(Mutex::new(Store::open(dir.path()).unwrap())),
            scope("parallel", false),
        )
        .unwrap(),
    );
    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let c = cache.clone();
            std::thread::spawn(move || c.inspect(&query(2)).unwrap().unwrap())
        })
        .collect();
    for task in tasks {
        assert_eq!(task.join().unwrap(), record(&query(2)));
    }
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
}
