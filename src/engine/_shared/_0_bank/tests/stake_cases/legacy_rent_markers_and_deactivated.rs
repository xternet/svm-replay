use super::*;

#[test]
fn legacy_rent_markers_and_deactivated_reward_types_are_era_gated() {
    let mut source = input(false);
    source["runtime"]["executorSourceId"] = json!("litesvm-v0.7.1-agave-2.3.9");
    for field in ["parentAccount", "targetAccount"] {
        change(&mut source[field], |v| {
            v["result"]["value"]["rentEpoch"] = json!(0)
        });
    }
    assert!(recover_untouched_reward_stake(&source).is_err());
    let mut source = input(false);
    change(&mut source["block"], |v| {
        v["result"]["rewards"][0]["rewardType"] = json!("DeactivatedStake")
    });
    assert!(recover_untouched_reward_stake(&source).is_err());
    source["runtime"]["executorSourceId"] = json!("litesvm-v0.16.0-agave-4.2.1");
    assert_eq!(
        recover_untouched_reward_stake(&source).unwrap()["proof"]["rewardType"],
        "DeactivatedStake"
    );
}

#[test]
fn exact_integer_boundaries_and_unproven_rent_overflow_remain_explicit() {
    let mut source = input(false);
    let old = 9_007_199_254_740_993u64;
    change(&mut source["parentAccount"], |v| {
        v["result"]["value"]["lamports"] = json!(old)
    });
    change(&mut source["targetAccount"], |v| {
        v["result"]["value"]["lamports"] = json!(old + 7)
    });
    change(&mut source["block"], |v| {
        v["result"]["rewards"][0]["postBalance"] = json!(old + 7)
    });
    assert_eq!(
        recover_untouched_reward_stake(&source).unwrap()["account"]["lamports"],
        (old + 7).to_string()
    );
    change(&mut source["targetAccount"], |v| {
        v["result"]["value"]["lamports"] = json!(old + 6)
    });
    assert!(recover_untouched_reward_stake(&source).is_err());
    for threshold in [1.0, 2.0, 1.5] {
        let value = adjusted(false, u64::MAX, threshold);
        assert_eq!(
            recover_untouched_reward_stake(&value).unwrap_err().code,
            "UNSUPPORTED_REWARD_RECOVERY"
        );
    }
    let mut source = input(false);
    change_bytes(&mut source["parentAccount"], |b| {
        b[156..164].copy_from_slice(&u64::MAX.to_le_bytes())
    });
    change_bytes(&mut source["targetAccount"], |b| {
        b[156..164].copy_from_slice(&u64::MAX.to_le_bytes())
    });
    assert!(recover_untouched_reward_stake(&source).is_err());
}

#[test]
#[ignore = "requires explicit SVM_REPLAY_TEST_STAKE_REFERENCE_MANIFEST pinned native adjustment reference"]
fn rent_adjustment_agrees_with_pinned_native_reference() {
    let path = std::path::PathBuf::from(
        std::env::var("SVM_REPLAY_TEST_STAKE_REFERENCE_MANIFEST")
            .expect("SVM_REPLAY_TEST_STAKE_REFERENCE_MANIFEST"),
    );
    let manifest: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let reference = &manifest["reference"];
    for file in reference["sources"]["files"].as_array().unwrap() {
        assert_eq!(
            hash(std::fs::read(file["path"].as_str().unwrap()).unwrap()),
            file["sha256"]
        );
    }
    let binary = reference["binary"].as_str().unwrap();
    assert_eq!(
        hash(std::fs::read(binary).unwrap()),
        reference["binarySha256"]
    );
    let mut sources = Vec::new();
    let mut args = Vec::new();
    for zero in [false, true] {
        for threshold in [1.0f64, 2.0, 1.5, 0.5, 0.0, -1.0, f64::NAN, f64::INFINITY] {
            for rate in [0u64, 6960, 1_000_000, 31] {
                sources.push(adjusted(zero, rate, threshold));
                args.push(json!({"rate":rate.to_string(),"thresholdBits":threshold.to_bits().to_string(),
            "stake":"5000","reward":if zero{"0"}else{"7"},"lamports":if zero{"10000"}else{"10007"},"deactivation":u64::MAX.to_string(),"epoch":"4"}));
            }
        }
    }
    let output = std::process::Command::new(binary)
        .arg(serde_json::to_string(&args).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let results: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(results.as_array().unwrap().len(), sources.len());
    for (mut source, expected) in sources.into_iter().zip(results.as_array().unwrap()) {
        let stake = expected["stake"].as_str().unwrap().parse::<u64>().unwrap();
        let deactivation = expected["deactivation"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        change_bytes(&mut source["targetAccount"], |b| {
            b[156..164].copy_from_slice(&stake.to_le_bytes());
            b[172..180].copy_from_slice(&deactivation.to_le_bytes());
        });
        let result = recover_untouched_reward_stake(&source).unwrap();
        assert_eq!(
            result["proof"]["rentAdjustmentProof"]["minimumBalance"],
            expected["minimumBalance"]
        );
        change_bytes(&mut source["targetAccount"], |b| b[156] ^= 1);
        assert!(recover_untouched_reward_stake(&source).is_err());
    }
}

#[test]
#[ignore = "requires SVM_REPLAY_TEST_STAKE_CAPTURE_ROOT and SVM_REPLAY_TEST_STAKE_TIMELINE immutable historical artifacts"]
fn three_preserved_stakes_recover_and_prepare_exact_images() {
    let root = std::path::PathBuf::from(
        std::env::var("SVM_REPLAY_TEST_STAKE_CAPTURE_ROOT")
            .expect("SVM_REPLAY_TEST_STAKE_CAPTURE_ROOT"),
    );
    let read = |path: &std::path::Path| -> Value {
        svm_replay_protocol::parse_json(&std::fs::read(path).unwrap()).unwrap()
    };
    let summary = read(&root.join("summary.json"));
    let timeline = read(std::path::Path::new(
        &std::env::var("SVM_REPLAY_TEST_STAKE_TIMELINE").expect("SVM_REPLAY_TEST_STAKE_TIMELINE"),
    ));
    let slot = summary["slot"].as_u64().unwrap();
    let profile = timeline["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| {
            v["earliestSlot"].as_u64().unwrap() <= slot && v["latestSlot"].as_u64().unwrap() >= slot
        })
        .unwrap();
    let receipt = |n: usize| read(&root.join(format!("rpc-{n:03}.json")));
    for (index, recorded) in summary["recovered"]
        .as_array()
        .unwrap()
        .iter()
        .take(3)
        .enumerate()
    {
        let source = json!({"pubkey":recorded["pubkey"],"boundary":{"slot":slot,"parentSlot":summary["parentSlot"],"blockhash":summary["blockhash"],
            "blockHeight":summary["blockHeight"],"transactionCount":summary["transactionCount"]},"runtime":profile,
            "block":receipt(5),"parentBlock":receipt(3),"parentAccount":receipt(6+index*2),"targetAccount":receipt(7+index*2)});
        let result = recover_untouched_reward_stake(&source).unwrap();
        assert_eq!(result["account"], recorded["after"]);
        assert_eq!(result["parentAccount"], recorded["before"]);
        let request = json!({"accounts":[result["parentAccount"]],"slot":slot,"parentSlot":summary["parentSlot"],"envelope":payload(&source["block"]),
            "blockSourceHash":source["block"]["responseSha256"],"runtime":profile,"evidence":{"block":source["block"],"parentBlock":source["parentBlock"],
                "accounts":[{"pubkey":source["pubkey"],"parentAccount":source["parentAccount"],"targetAccount":source["targetAccount"]}]}});
        assert_eq!(
            prepare_stake_initialization(&request).unwrap()["accounts"],
            json!([recorded["after"]])
        );
    }
}
