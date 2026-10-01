use super::*;

#[test]
fn bank_context_recomputes_witness_and_insertion_order_lineage_hashes() {
    let (block, selector, source) = block_context();
    let context = derive_historical_bank_context(&block, &selector, &source, &[]).unwrap();
    assert_eq!(context["lamportsPerSignature"], 5);
    assert_eq!(context["feeWitnessCount"], 2);
    assert_eq!(
        context["feeWitnessHash"],
        hash(
            r#"[{"transactionIndex":0,"fee":10,"signatures":2},{"transactionIndex":1,"fee":5,"signatures":1}]"#
        )
    );
    let lineage=format!("{{\"sourceHash\":\"{source}\",\"targetSlot\":20,\"parentSlot\":19,\"blockhash\":\"{}\",\"previousBlockhash\":\"{}\"}}",key(3),key(4));
    assert_eq!(context["blockLineageHash"], hash(lineage));
    assert_eq!(context["feeWitnessIndicesHash"], hash("[0,1]"));
    assert_eq!(
        context["derivationHash"],
        historical_bank_context_hash(&context).unwrap()
    );
    for mutation in [
        "fee",
        "fractional",
        "no-witness",
        "selector",
        "blockhash",
        "overlap",
    ] {
        let (mut block, mut selector, source) = block_context();
        let mut indices = vec![];
        match mutation {
            "fee" => block["result"]["transactions"][1]["meta"]["fee"] = json!(6),
            "fractional" => block["result"]["transactions"][0]["meta"]["fee"] = json!(9),
            "no-witness" => {
                for tx in block["result"]["transactions"].as_array_mut().unwrap() {
                    tx["meta"]["err"] = json!("failed");
                }
            }
            "selector" => selector["signature"] = json!(key(6)),
            "blockhash" => block["result"]["previousBlockhash"] = json!("invalid"),
            "overlap" => indices.push(2),
            _ => unreachable!(),
        }
        assert!(
            derive_historical_bank_context(&block, &selector, &source, &indices).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn nonce_requirement_only_applies_to_outer_canonical_advance_position() {
    let (mut block, selector, source) = block_context();
    let message = &mut block["result"]["transactions"][2]["transaction"]["message"];
    message["accountKeys"]
        .as_array_mut()
        .unwrap()
        .push(json!("SysvarRecentB1ockHashes11111111111111111111"));
    message["instructions"][0]["accounts"] = json!([0, 2]);
    message["instructions"][0]["data"] = json!(bs58::encode([4, 0, 0, 0]).into_string());
    let context = derive_historical_bank_context(&block, &selector, &source, &[]).unwrap();
    assert_eq!(
        context["requiredRuntimeSysvars"],
        json!([{"pubkey":"SysvarRecentB1ockHashes11111111111111111111","requirement":"nonce-advance-nonempty-cache-v1"}])
    );
    block["result"]["transactions"][2]["transaction"]["message"]["instructions"][0]["accounts"] =
        json!([2, 0]);
    assert_eq!(
        derive_historical_bank_context(&block, &selector, &source, &[]).unwrap()
            ["requiredRuntimeSysvars"],
        json!([])
    );
}

#[test]
fn clock_and_generic_bindings_require_exact_target_bytes() {
    let strict = clock(20);
    let rpc = json!({"pubkey":CLOCK_SYSVAR,"value":{"owner":strict["owner"],"executable":false,"lamports":1,
        "rentEpoch":u64::MAX,"data":[strict["dataBase64"],"base64"]}});
    assert_eq!(build_target_clock(&rpc, 20).unwrap(), strict);
    let binding = bind_exact_generic_sysvar(&strict, 20).unwrap();
    assert_eq!(binding["dataLen"], 40);
    for field in [
        "sourceSlot",
        "owner",
        "presence",
        "executable",
        "dataBase64",
    ] {
        let mut changed = strict.clone();
        changed[field] = Value::Null;
        assert!(bind_exact_generic_sysvar(&changed, 20).is_err(), "{field}");
    }
    assert!(build_target_clock(&rpc, 21).is_err());
    let mut malformed = rpc;
    malformed["value"]["data"][0] = json!("%%% ");
    assert!(build_target_clock(&malformed, 20).is_err());
}

#[test]
fn bank_initialized_images_are_exact_copies_not_reward_or_rent_reconstruction() {
    let rent = "SysvarRent111111111111111111111111111111111";
    let parent = vec![account(rent, 19, &[1, 2]), account(&key(9), 19, &[7])];
    let targets = vec![account(rent, 20, &[3, 4])];
    let result = bind_bank_initialized_sysvars(&parent, &targets, 20).unwrap();
    assert_eq!(result["accounts"][0], targets[0]);
    assert_eq!(result["accounts"][1], parent[1]);
    assert_bank_initialized_sysvar_bindings(
        result["accounts"].as_array().unwrap(),
        20,
        result["bindings"].as_array().unwrap(),
    )
    .unwrap();
    let mut changed = parent.clone();
    changed[0]["rentEpoch"] = json!("0");
    assert!(bind_bank_initialized_sysvars(&changed, &targets, 20).is_err());
    assert!(bind_bank_initialized_sysvars(&parent, &[], 20).is_err());
    let mut changed = result["bindings"].as_array().unwrap().clone();
    changed[0]["phasePolicy"] = json!("end-of-slot");
    assert!(assert_bank_initialized_sysvar_bindings(
        result["accounts"].as_array().unwrap(),
        20,
        &changed
    )
    .is_err());
}

#[test]
fn restart_binding_is_derived_from_exact_raw_receipt_without_network() {
    let transcript = transcript();
    let before = transcript.clone();
    let result = prepare_last_restart_slot(&transcript, 19).unwrap();
    assert_eq!(result["binding"]["embeddedSlot"], "17");
    assert_last_restart_slot_binding(&result["account"], &result["binding"], 19).unwrap();
    assert_eq!(transcript, before);
    assert_eq!(result["transcript"], transcript);
    for field in ["responseBodyBase64", "responseBytes", "responseSha256"] {
        let mut changed = transcript.clone();
        changed[field] = Value::Null;
        assert!(prepare_last_restart_slot(&changed, 19).is_err(), "{field}");
    }
    assert!(prepare_last_restart_slot(&transcript, 20).is_err());
    let mut changed = result["binding"].clone();
    changed["rawResponseSha256"] = json!(hash(b"other receipt"));
    assert!(assert_last_restart_slot_binding(&result["account"], &changed, 19).is_err());
}

#[test]
fn source_restart_projection_is_explicit_and_binds_exact_retained_account() {
    let source = account(LAST_RESTART_SLOT, 19, &17u64.to_le_bytes());
    let evidence = vec![hash(b"retained source observation")];
    let result = prepare_last_restart_from_observation(&source, &evidence).unwrap();
    assert_eq!(result["account"], source);
    assert_eq!(result["provenance"]["kind"], "local-source-projection/v1");
    assert_eq!(result["provenance"]["evidenceHashes"], json!(evidence));
    assert_eq!(result["provenance"]["originalProviderResponse"], false);
    assert_eq!(result["cache"]["status"], "LOCAL_SOURCE_PROJECTION");
    assert_last_restart_slot_binding(&source, &result["binding"], 19).unwrap();
    assert!(prepare_last_restart_from_observation(&source, &[]).is_err());
    assert!(prepare_last_restart_from_observation(&source, &["invalid".into()]).is_err());
    for (field, value) in [
        ("owner", json!(key(5))),
        ("role", json!("application")),
        ("presence", json!("absent")),
        ("lamports", json!(1)),
        ("dataBase64", json!("AQ==")),
        ("extra", json!(true)),
    ] {
        let mut changed = source.clone();
        changed[field] = value;
        assert!(
            prepare_last_restart_from_observation(&changed, &evidence).is_err(),
            "{field}"
        );
    }
}
