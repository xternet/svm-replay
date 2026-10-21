use super::*;

#[test]
fn slot_position_accepts_an_empty_produced_block_without_any_transaction() {
    let block = json!({"result":{"parentSlot":10,"transactions":[]}});
    assert_eq!(position(&block, 11, 14).unwrap(), 10);
    assert!(position(&block, 9, 14).is_err());
    assert!(position(&block, 14, 14).is_err());
    assert!(position(&block, 11, 9_007_199_254_740_992).is_err());
}

#[test]
fn slot_position_never_selects_or_requires_an_original_transaction() {
    let block = json!({"result":{"parentSlot":10,"transactions":[{"transaction":"not used as an original"}]}});
    assert_eq!(position(&block, 10, 11).unwrap(), 10);
    assert!(position(&json!({"result":{"parentSlot":10}}), 10, 11).is_err());
}

fn request() -> InitializationRequest {
    use crate::_1_resolve_runtime::registry::Registry;
    use svm_replay_protocol::worker::WorkerDescriptor;
    let registry = Registry::bundled().unwrap();
    let slot = registry.window.earliest_slot;
    let profile = registry.profile(slot).unwrap();
    let document: Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/_1_resolve_runtime/_2_registry/catalog.json"
    )))
    .unwrap();
    let executor = document["executors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == profile.executor_source_id)
        .unwrap();
    let worker = WorkerDescriptor {
        family: executor["packageName"].as_str().unwrap().into(),
        file: "not-executed-by-metadata-unit-tests".into(),
        sha256: Digest::of(b"unit worker"),
        build_hash: Digest::of(b"unit build"),
        source_sha256: Digest::new(executor["reviewedSourceSha256"].as_str().unwrap()).unwrap(),
        executor_source_id: profile.executor_source_id.clone(),
        capabilities: vec!["guarded-generic-sysvars/v1".into()],
    };
    let raw =
        serde_json::to_vec(&json!({"result":{"parentSlot":slot-1,"transactions":[]}})).unwrap();
    InitializationRequest {
        schema: "svm-historical-initialization-request/v1".into(),
        request_id: "unit-slot".into(),
        family: worker.family.clone(),
        genesis_hash: bs58::encode([1u8; 32]).into_string(),
        after_slot: slot - 1,
        slot,
        raw_block_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &raw),
        block_sha256: Digest::of(raw),
        runtime_binding: registry.bind(slot, &worker).unwrap(),
        extra_accounts: vec![],
        limits: Limits {
            timeout_ms: 1000,
            max_output_bytes: 1024 * 1024,
            max_diagnostic_bytes: 1024,
        },
    }
}

#[test]
fn initialized_request_binds_raw_bytes_runtime_slot_and_account_scope() {
    let original = request();
    assert!(original.validate().is_ok());
    let mut changed = original.clone();
    changed.after_slot = changed.slot;
    assert!(changed.validate().is_err());
    let mut changed = original.clone();
    changed.raw_block_base64 = "e30=".into();
    assert!(changed.validate().is_err());
    let mut changed = original.clone();
    changed.slot += 1;
    assert!(changed.validate().is_err());
    let mut changed = original.clone();
    changed.extra_accounts = vec!["11111111111111111111111111111111".into(); 2];
    assert!(changed.validate().is_err());
    let mut changed = original;
    changed.extra_accounts = vec!["invalid".into()];
    assert!(changed.validate().is_err());
}

#[test]
fn initialized_request_rejects_archived_transaction_fields_instead_of_ignoring_them() {
    let original = serde_json::to_value(request()).unwrap();
    for field in [
        "signature",
        "candidate",
        "transactionBase64",
        "original",
        "after",
    ] {
        let mut changed = original.clone();
        changed[field] = json!("not an initialized-slot field");
        assert!(
            serde_json::from_value::<InitializationRequest>(changed).is_err(),
            "{field}"
        );
    }
}
