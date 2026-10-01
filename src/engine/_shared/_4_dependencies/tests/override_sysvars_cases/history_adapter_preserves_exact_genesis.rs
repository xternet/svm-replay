use super::*;

#[test]
fn history_adapter_preserves_exact_genesis_phase_slots_and_cumulative_limits() {
    use svm_replay_engine::shared::{
        history::History,
        runtime::{CancellationToken, ExecutionBudget},
        sources::{CompositeSource, HistoricalSource, SourceError},
    };
    use svm_replay_protocol::Digest;
    struct Source;
    let genesis = bs58::encode([1u8; 32]).into_string();
    impl HistoricalSource for Source {
        fn identity(&self) -> Value {
            json!({"id":"override-phase-test","version":"1","genesisHash":bs58::encode([1u8;32]).into_string(),"kind":"captured-history","coverage":{"firstSlot":97,"lastSlot":100,"completeness":"partial"},"capabilities":["account"]})
        }
        fn inspect(&self, query: &Value) -> Result<Option<Value>, SourceError> {
            let key = query["pubkey"].as_str().expect("query key");
            let slot = query["slot"].as_u64().expect("query slot");
            assert_eq!(query["phase"], "end-slot");
            assert_eq!(slot, if key == SLOT_HASHES { 100 } else { 97 });
            let value = account(key, slot);
            Ok(Some(
                json!({"query":query,"value":value,"evidenceHashes":[Digest::of(serde_json::to_vec(&value).unwrap())]}),
            ))
        }
    }
    let mut source = CompositeSource::new(vec![Box::new(Source)]).unwrap();
    let budget =
        ExecutionBudget::new(std::time::Duration::from_secs(10), CancellationToken::new()).unwrap();
    let mut history = History::new(&mut source, genesis, 100, &budget, 2).unwrap();
    let features = json!([]);
    let c = context(&features);
    let output =
        hydrate_override_sysvar_baselines(&edits(&[LEGACY[0], SLOT_HASHES]), &[], &c, &mut history)
            .unwrap();
    assert_eq!(
        output.accounts,
        vec![account(LEGACY[0], 97), account(SLOT_HASHES, 100)]
    );
    assert_eq!(
        hydrate_override_sysvar_baselines(&edits(&[LEGACY[1]]), &[], &c, &mut history)
            .unwrap_err()
            .code,
        "UNSUPPORTED_RESOURCE_LIMIT"
    );
    let mut history = History::new(
        &mut source,
        bs58::encode([9u8; 32]).into_string(),
        100,
        &budget,
        2,
    )
    .unwrap();
    assert_eq!(
        hydrate_override_sysvar_baselines(&edits(&[LEGACY[0]]), &[], &c, &mut history)
            .unwrap_err()
            .code,
        "SOURCE_CONTEXT_MISMATCH"
    );
    let wrong = OverrideSysvarContext {
        target_slot: 101,
        ..c
    };
    assert_eq!(
        hydrate_override_sysvar_baselines(&edits(&[LEGACY[0]]), &[], &wrong, &mut history)
            .unwrap_err()
            .code,
        "SOURCE_CONTEXT_MISMATCH"
    );
}

#[test]
fn fetched_wrong_source_slot_and_invalid_boundary_fail_without_relabeling() {
    let features = json!([]);
    let c = context(&features);
    let error = hydrate_override_sysvar_baselines_with_accounts(
        &edits(&[SLOT_HASHES]),
        &[],
        &c,
        |key, _| Ok(vec![account(key, 97)]),
    )
    .unwrap_err();
    assert_eq!(error.code, "SOURCE_CONTEXT_MISMATCH");
    for (parent_slot, target_slot) in [(100, 100), (101, 100), (97, 9_007_199_254_740_992)] {
        let c = OverrideSysvarContext {
            parent_slot,
            target_slot,
            ..context(&features)
        };
        assert!(hydrate_override_sysvar_baselines_with_accounts(
            &edits(&LEGACY),
            &[],
            &c,
            |_, _| panic!("invalid boundary fetch")
        )
        .is_err());
    }
}

#[test]
fn explicit_feature_context_and_tower_source_proof_are_required_only_for_tracked_overrides() {
    for features in [
        Value::Null,
        json!("bad"),
        json!([1]),
        json!(["a1penGLz8Vm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP"]),
    ] {
        let c = context(&features);
        let e =
            hydrate_override_sysvar_baselines_with_accounts(&edits(&LEGACY), &[], &c, |_, _| {
                panic!("invalid phase must not fetch")
            })
            .unwrap_err();
        assert_eq!(e.code, "UNSUPPORTED_RUNTIME_PROFILE");
        let unrelated = hydrate_override_sysvar_baselines_with_accounts(
            &edits(&["SysvarC1ock11111111111111111111111111111111"]),
            &[],
            &c,
            |_, _| panic!("unrelated fetch"),
        )
        .unwrap();
        assert!(unrelated.bindings.is_empty());
    }
    let features = json!([]);
    let c = OverrideSysvarContext {
        executor_source_id: "unreviewed",
        ..context(&features)
    };
    assert_eq!(
        hydrate_override_sysvar_baselines_with_accounts(&edits(&LEGACY), &[], &c, |_, _| panic!(
            "source fetch"
        ))
        .unwrap_err()
        .code,
        "UNSUPPORTED_RUNTIME_PROFILE"
    );
    assert_eq!(
        hydrate_override_sysvar_baselines_with_accounts(
            &edits(&["Sysvar1nstructions1111111111111111111111111"]),
            &[],
            &c,
            |_, _| panic!("instructions fetch")
        )
        .unwrap_err()
        .message,
        "RUNTIME_DERIVED_SYSVAR_OVERRIDE"
    );
}
