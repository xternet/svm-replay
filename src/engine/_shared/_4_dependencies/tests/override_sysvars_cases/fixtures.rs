use super::*;

pub(super) const SLOT_HASHES: &str = "SysvarS1otHashes111111111111111111111111111";

pub(super) const LEGACY: [&str; 4] = [
    "SysvarS1otHistory11111111111111111111111111",
    "SysvarFees111111111111111111111111111111111",
    "SysvarRecentB1ockHashes11111111111111111111",
    "SysvarRewards111111111111111111111111111111",
];

pub(super) const EXECUTORS: [&str; 6] = [
    "litesvm-v0.7.1-agave-2.3.9",
    "litesvm-v0.8.2-agave-3.0.10",
    "litesvm-v0.12.0-agave-3.1.11",
    "litesvm-v0.13.1-agave-4.0.0",
    "litesvm-v0.14.0-pr402-agave-4.1.2",
    "litesvm-v0.16.0-agave-4.2.1",
];

pub(super) fn account(key: &str, slot: u64) -> Value {
    json!({"pubkey":key,"sourceSlot":slot,"role":"sysvar","presence":"present","lamports":"9007199254740993","owner":"Sysvar1111111111111111111111111111111111111","executable":false,"rentEpoch":u64::MAX.to_string(),"dataBase64":STANDARD.encode(format!("controlled:{key}:{slot}"))})
}

pub(super) fn edits(keys: &[&str]) -> Vec<RequestedAccountOverride> {
    keys.iter()
        .map(|key| RequestedAccountOverride {
            pubkey: (*key).into(),
            lamports: Some("12345678".into()),
            data_base64: None,
        })
        .collect()
}

pub(super) fn context(features: &Value) -> OverrideSysvarContext<'_> {
    OverrideSysvarContext {
        parent_slot: 97,
        target_slot: 100,
        executor_source_id: EXECUTORS[0],
        active_execution_feature_ids: features,
    }
}
