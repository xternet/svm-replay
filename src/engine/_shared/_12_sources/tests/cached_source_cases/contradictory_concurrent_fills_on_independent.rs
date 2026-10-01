use super::*;

#[test]
fn contradictory_concurrent_fills_on_independent_handles_fail_immutable_conflict() {
    use std::sync::Barrier;
    struct Racing {
        barrier: Arc<Barrier>,
        lamports: u64,
    }
    impl HistoricalSource for Racing {
        fn identity(&self) -> Value {
            identity()
        }
        fn inspect(&self, q: &Value) -> Result<Option<Value>, SourceError> {
            self.barrier.wait();
            let a = json!({"pubkey":q["pubkey"],"sourceSlot":q["slot"],"role":"application","presence":"present","lamports":self.lamports.to_string(),"owner":key(0),"executable":false,"rentEpoch":"0","dataBase64":""});
            Ok(Some(
                json!({"query":q,"value":a,"evidenceHashes":[Digest::of(serde_json::to_vec(&a).unwrap())]}),
            ))
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let mut caches = vec![];
    for lamports in [1, 2] {
        caches.push(
            CachedSource::new(
                Arc::new(Racing {
                    barrier: barrier.clone(),
                    lamports,
                }),
                Arc::new(Mutex::new(Store::open(dir.path()).unwrap())),
                scope("race", false),
            )
            .unwrap(),
        );
    }
    let tasks: Vec<_> = caches
        .into_iter()
        .map(|cache| std::thread::spawn(move || cache.inspect(&query(2))))
        .collect();
    let results: Vec<_> = tasks.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|v| v.is_ok()).count(), 1);
    let error = results.iter().find_map(|v| v.as_ref().err()).unwrap();
    assert_eq!(error.code, "SOURCE_CONFLICT");
}

#[test]
fn corrupted_content_blob_is_an_error_and_never_a_cache_miss() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
    let source = Source::new();
    let options = scope("blob", false);
    let key = raw_cache_key(&options, &identity(), &query(2)).unwrap();
    let cache = CachedSource::new(source.clone(), store.clone(), options).unwrap();
    cache.inspect(&query(2)).unwrap();
    let hit = store
        .lock()
        .unwrap()
        .execute(json!({"version":1,"op":"get","namespace":"raw","key":key}))
        .unwrap();
    let hash = hit["sha256"].as_str().unwrap();
    std::fs::write(
        dir.path().join("blobs").join(hash),
        b"deliberately corrupted test blob",
    )
    .unwrap();
    assert_eq!(
        cache.inspect(&query(2)).unwrap_err().code,
        "SOURCE_CACHE_ERROR"
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
}
