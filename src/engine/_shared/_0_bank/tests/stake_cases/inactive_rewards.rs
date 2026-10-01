use super::*;

fn inactive() -> Value {
    let mut value = prepared(&input(true));
    value.as_object_mut().unwrap().remove("evidence");
    value["envelope"]["result"]["transactions"] = json!([instruction("writer")]);
    let mut data = vec![0u8; 81];
    data[..8].copy_from_slice(&5u64.to_le_bytes());
    data[8..16].copy_from_slice(&4u64.to_le_bytes());
    value["inactiveRewards"] = json!({"account":{
        "pubkey":"SysvarEpochRewards1111111111111111111111111","sourceSlot":20,
        "role":"sysvar","presence":"present","owner":"Sysvar1111111111111111111111111111111111111",
        "executable":false,"lamports":"1","rentEpoch":u64::MAX.to_string(),
        "dataBase64":STANDARD.encode(data)},"evidenceHashes":[hash("captured test input")]});
    value
}

#[test]
fn completed_reward_interval_preserves_parent_stake_before_successful_writes() {
    let value = inactive();
    let result = prepare_stake_initialization(&value).unwrap();
    assert_eq!(result["accounts"], value["accounts"]);
    assert_eq!(
        result["proofs"][0]["schema"],
        "svm-simulate-inactive-stake-initialization/v1"
    );
    assert_eq!(result["proofs"][0]["distributionEndExclusive"], 9);
    let mut initialized = value.clone();
    let mut bytes = STANDARD
        .decode(initialized["accounts"][0]["dataBase64"].as_str().unwrap())
        .unwrap();
    bytes[..4].copy_from_slice(&1u32.to_le_bytes());
    initialized["accounts"][0]["dataBase64"] = json!(STANDARD.encode(bytes));
    assert_eq!(
        prepare_stake_initialization(&initialized).unwrap()["accounts"],
        initialized["accounts"]
    );
}

#[test]
fn active_final_distribution_wrong_era_or_unbound_sysvar_never_pass() {
    for (pointer, bad) in [
        (
            "/runtime/executorSourceId",
            json!("unreviewed-future-executor"),
        ),
        ("/envelope/result/blockHeight", json!(8)),
        ("/inactiveRewards/account/sourceSlot", json!(19)),
        ("/inactiveRewards/account/owner", json!(key(7))),
        ("/inactiveRewards/evidenceHashes", json!([])),
        ("/accounts/0/sourceSlot", json!(18)),
    ] {
        let mut value = inactive();
        *value.pointer_mut(pointer).unwrap() = bad;
        assert!(prepare_stake_initialization(&value).is_err(), "{pointer}");
    }
    for (offset, byte) in [(80, 1), (80, 2), (8, 100)] {
        let mut value = inactive();
        let mut data = STANDARD
            .decode(
                value["inactiveRewards"]["account"]["dataBase64"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
        data[offset] = byte;
        value["inactiveRewards"]["account"]["dataBase64"] = json!(STANDARD.encode(data));
        assert!(prepare_stake_initialization(&value).is_err());
    }
}

#[test]
fn inactive_stake_initialization_supports_reviewed_newer_eras() {
    for executor in [
        "litesvm-v0.14.0-pr402-agave-4.1.2",
        "litesvm-v0.16.0-agave-4.2.1",
    ] {
        let mut value = inactive();
        value["runtime"]["executorSourceId"] = json!(executor);
        let result = prepare_stake_initialization(&value).unwrap();
        assert_eq!(result["accounts"], value["accounts"]);
        assert_eq!(result["proofs"][0]["executorSourceId"], executor);
        value["envelope"]["result"]["blockHeight"] = json!(8);
        assert!(prepare_stake_initialization(&value).is_err());
        value["envelope"]["result"]["blockHeight"] = json!(9);
        value["runtime"]["activeExecutionFeatureIds"] =
            json!(["a1penGLz8Vm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP"]);
        assert!(prepare_stake_initialization(&value).is_err());
    }
}

#[test]
fn inactive_stake_initialization_supports_reviewed_legacy_eras() {
    for (executor, commit) in [
        (
            "litesvm-v0.6.1-agave-2.2.20",
            "dabc99a539c4b024a715cb247a4decf7e45d8659",
        ),
        (
            "litesvm-v0.7.1-agave-2.3.9",
            "47647df756f5dd0b3c739cecaa71bcf754af6be8",
        ),
        (
            "litesvm-v0.8.2-agave-3.0.10",
            "96c3a8519a3bac8c7e7dd49b6d6aefcfeba09d90",
        ),
        (
            "litesvm-v0.12.0-agave-3.1.11",
            "81d1f5fb0c75203329468250df3769393ab1479b",
        ),
    ] {
        let mut value = inactive();
        value["runtime"]["executorSourceId"] = json!(executor);
        value["accounts"][0]["rentEpoch"] = json!(u64::MAX.to_string());
        let result = prepare_stake_initialization(&value).unwrap();
        assert_eq!(result["accounts"], value["accounts"]);
        assert_eq!(result["proofs"][0]["phaseSourceCommit"], commit);
        if executor.contains("2.3.9") || executor.contains("2.2.20") {
            value["accounts"][0]["rentEpoch"] = json!("1");
            assert!(prepare_stake_initialization(&value).is_err());
        }
    }
}
