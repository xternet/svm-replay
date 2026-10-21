use super::*;
fn address(n: u8) -> String {
    bs58::encode([n; 32]).into_string()
}
#[test]
fn feature_binding_rejects_duplicates_future_bad_keys_and_slots() {
    let id = address(1);
    for value in [
        Value::Null,
        json!([{"id":id,"activationSlot":301}]),
        json!([{"id":id,"activationSlot":-1}]),
        json!([{"id":id,"activationSlot":1.5}]),
        json!([{"id":"1111","activationSlot":0}]),
        json!([{"id":id,"activationSlot":0},{"id":id,"activationSlot":0}]),
    ] {
        assert!(_3_features::binding(&value, 300).is_err());
    }
    assert_eq!(
        _3_features::binding(&json!([]), 300).unwrap(),
        Vec::<Value>::new()
    );
}
#[test]
fn feature_balances_use_historical_rent_with_truncation_and_exact_u64s() {
    let features = vec![json!({"id":address(1),"activationSlot":299})];
    for (rate, threshold, expected) in [(7000_u64, 1.5_f64, 1_438_500_u64), (3, 1.25, 513)] {
        let mut data = rate.to_le_bytes().to_vec();
        data.extend(threshold.to_le_bytes());
        data.push(50);
        let accounts =
            json!({RENT:{"owner":SYSVAR,"executable":false,"dataBase64":STANDARD.encode(data)}});
        let images = _3_features::derive(&features, accounts.as_object().unwrap()).unwrap();
        assert_eq!(images[&address(1)]["lamports"], expected.to_string());
        assert_eq!(images[&address(1)]["rentEpoch"], u64::MAX.to_string());
        assert_eq!(
            bytes(&images[&address(1)]["dataBase64"]).unwrap(),
            [1, 43, 1, 0, 0, 0, 0, 0, 0]
        );
    }
}
#[test]
fn feature_reads_reject_absent_malformed_and_overflowing_rent() {
    let features = vec![json!({"id":address(1),"activationSlot":1})];
    for accounts in [
        json!({}),
        json!({RENT:{"owner":SYSVAR,"executable":false,"dataBase64":"AA=="}}),
    ] {
        assert!(_3_features::derive(&features, accounts.as_object().unwrap()).is_err());
    }
    let mut data = u64::MAX.to_le_bytes().to_vec();
    data.extend(2_f64.to_le_bytes());
    data.push(50);
    assert!(_3_features::derive(
        &features,
        json!({RENT:{"owner":SYSVAR,"executable":false,"dataBase64":STANDARD.encode(data)}})
            .as_object()
            .unwrap()
    )
    .is_err());
}
#[test]
fn companions_only_follow_actual_historical_data_and_skip_known_states() {
    let mint = address(2);
    let pd = address(3);
    let mut program = vec![2, 0, 0, 0];
    program.extend([3; 32]);
    let mut token = vec![2; 32];
    token.resize(165, 0);
    let b = json!({"accounts":{address(5):{"owner":UPGRADEABLE,"executable":true,"dataBase64":STANDARD.encode(program)},
        address(6):{"owner":"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA","executable":false,"dataBase64":STANDARD.encode(token)}},"absent":[mint],"unavailable":[]});
    assert_eq!(_4_expansion::companions(&b).unwrap(), vec![pd]);
}
#[test]
fn projection_refuses_empty_and_unbounded_job_lists() {
    for jobs in [json!([]), json!([{}, {}, {}, {}, {}, {}])] {
        assert!(project(
            &json!({"schema":"svm-replay-boundary-request/v1","jobs":jobs,"extra":[]})
        )
        .is_err());
    }
}
#[test]
fn footprint_preserves_payer_blockhash_and_declares_only_requested_readonly_accounts() {
    let mut wire = vec![1];
    wire.extend([0; 64]);
    wire.extend([1, 0, 0, 1]);
    wire.extend([1; 32]);
    wire.extend([9; 32]);
    wire.push(0);
    let encoded = STANDARD.encode(wire);
    let expanded = _4_expansion::footprint(&encoded, &[address(2), address(3)]).unwrap();
    let decoded = svm_replay_protocol::transaction::decode(&expanded).unwrap();
    let message = &decoded["transaction"]["message"];
    assert_eq!(
        message["accountKeys"],
        json!([address(1), address(2), address(3)])
    );
    assert_eq!(message["recentBlockhash"], address(9));
    assert_eq!(message["instructions"], json!([]));
    assert_eq!(message["header"]["numReadonlyUnsignedAccounts"], 2);
    assert!(_4_expansion::footprint(&encoded, &[]).is_err());
    assert!(_4_expansion::footprint(&encoded, &vec![address(2); 201]).is_err());
    assert!(_4_expansion::footprint(&encoded, &["bad".to_owned()]).is_err());
    assert!(_4_expansion::footprint("AA==", &[address(2)]).is_err());
}
#[test]
fn touched_accounts_include_actual_writes_and_omitted_fees_not_readonly_loads() {
    let job = Job {
        prepared: json!({"fixture":{"target":{"omittedFeeEffects":[{"feePayer":"C"}]}}}),
        result: json!({"prefix":[{"accountTransitions":[{"pubkey":"A","before":1,"after":2},{"pubkey":"B","before":3,"after":3}]}]}),
        journal: json!([]),
        path: String::new(),
        runtime: Value::Null,
    };
    assert_eq!(
        _2_history::touched(&job).unwrap(),
        BTreeSet::from(["A".to_owned(), "C".to_owned()])
    );
}
#[test]
fn parent_images_preserve_proven_absence_and_reject_unknown_presence() {
    let mut job = Job {
        prepared: json!({"fixture":{"accounts":[{"pubkey":address(1),"presence":"absent"}]}}),
        result: Value::Null,
        journal: Value::Null,
        path: String::new(),
        runtime: Value::Null,
    };
    assert_eq!(
        _2_history::parents(&job).unwrap().get(&address(1)),
        Some(&None)
    );
    job.prepared["fixture"]["accounts"][0]["presence"] = json!("maybe");
    assert!(_2_history::parents(&job).is_err());
}
#[test]
fn token2022_companions_require_account_type_and_skip_known_or_nonupgradeable_programs() {
    let mut data = vec![2; 32];
    data.resize(167, 0);
    data[165] = 2;
    let mut b = json!({"accounts":{address(5):{"owner":"TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb","executable":false,"dataBase64":STANDARD.encode(&data)},
        address(6):{"owner":"BPFLoader2111111111111111111111111111111111","executable":true,"dataBase64":""}},"absent":[],"unavailable":[]});
    assert_eq!(_4_expansion::companions(&b).unwrap(), vec![address(2)]);
    data[165] = 1;
    b["accounts"][address(5)]["dataBase64"] = json!(STANDARD.encode(data));
    assert!(_4_expansion::companions(&b).unwrap().is_empty());
}
#[test]
fn job_loader_requires_original_control_journal_and_verification_not_just_valid_hashes() {
    let root = tempfile::tempdir().unwrap();
    let pin = |name: &str, value: &Value| {
        let raw = serde_json::to_vec(value).unwrap();
        std::fs::write(root.path().join(name), &raw).unwrap();
        json!({"file":name,"sha256":Digest::of(&raw)})
    };
    let trace = pin("trace.json", &json!([]));
    let mut receipt = json!({"outcome":"COMPLETED","preparedRequest":pin("prepared.json",&json!({})),"output":pin("result.json",&json!({})),
        "controlVerification":{"status":"PASS"},"verification":{"status":"PASS"},"trace":{"exports":[{"phase":"requested","file":trace["file"],"sha256":trace["sha256"]}]}});
    let descriptor = |receipt: &Value, trace: &Value| {
        let raw = serde_json::to_vec(receipt).unwrap();
        let path = root.path().join("receipt.json");
        std::fs::write(&path, &raw).unwrap();
        json!({"receiptPath":path,"receiptSha256":Digest::of(&raw),"trace":trace})
    };
    assert!(
        Job::load(&descriptor(&receipt, &trace)).is_err(),
        "requested trace is not original control"
    );
    receipt["trace"]["exports"][0]["phase"] = json!("original-control");
    assert!(Job::load(&descriptor(&receipt, &trace)).is_ok());
    receipt["verification"]["status"] = json!("FAIL");
    assert!(Job::load(&descriptor(&receipt, &trace)).is_err());
    receipt["verification"]["status"] = json!("PASS");
    let other = pin("other.json", &json!([]));
    assert!(
        Job::load(&descriptor(&receipt, &other)).is_err(),
        "unbound journal is not authoritative"
    );
}
