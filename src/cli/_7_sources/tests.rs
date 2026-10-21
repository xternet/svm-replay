use super::*;
use serde_json::{json, Value};

#[test]
fn uncached_shared_provider_keeps_block_discovery_and_its_errors() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use svm_replay_engine::shared::sources::SourceError;
    struct Discovery(Arc<AtomicUsize>);
    impl HistoricalSource for Discovery {
        fn identity(&self) -> Value {
            json!({"id":"discovery-test","version":"1","genesisHash":"11111111111111111111111111111111",
                "kind":"captured-history","coverage":{"firstSlot":0,"lastSlot":10,"completeness":"partial"},"capabilities":["block"]})
        }
        fn inspect(&self, _: &Value) -> Result<Option<Value>, SourceError> {
            panic!("discovery must not become an unpinned inspect");
        }
        fn discover_block(&self, slot: u64) -> Result<Option<Value>, SourceError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(SourceError::new(
                "SOURCE_RPC_ERROR",
                format!("recorded provider failure at {slot}"),
            ))
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let directory = tempfile::tempdir().unwrap();
    let mut source = finish_sources(
        vec![(Arc::new(Discovery(calls.clone())), "test".into())],
        directory.path(),
        CacheMode::Off,
    )
    .unwrap()
    .unwrap();
    let error = source.discover_block(2).unwrap_err();
    assert_eq!(error.code, "SOURCE_RPC_ERROR");
    assert_eq!(error.message, "recorded provider failure at 2");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[test]
fn public_live_configuration_carries_budget_id_not_a_secret_or_endpoint() {
    let base = json!({"id":"alchemy-history","version":"v1","genesisHash":"explicit-network",
        "firstSlot":1,"lastSlot":2,"maxRequests":2,"maxAccountReads":1,
        "maxDownloadBytes":1024,"maxResponseBytes":256});
    let parsed: LiveConfig = serde_json::from_value(base.clone()).unwrap();
    assert!(parsed.durable_budget_id.is_none());
    let mut durable = base.clone();
    durable["durableBudgetId"] = json!("operator-selected-run");
    assert_eq!(
        serde_json::from_value::<LiveConfig>(durable)
            .unwrap()
            .durable_budget_id
            .as_deref(),
        Some("operator-selected-run")
    );
    for field in ["apiKey", "endpoint", "retries"] {
        let mut unexpected = base.clone();
        unexpected[field] = json!("must-not-be-accepted");
        assert!(serde_json::from_value::<LiveConfig>(unexpected).is_err());
    }
}
