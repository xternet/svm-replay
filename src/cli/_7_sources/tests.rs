use super::*;
use serde_json::json;
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
