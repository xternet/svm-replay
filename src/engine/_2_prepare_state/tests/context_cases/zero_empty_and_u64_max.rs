use super::*;

#[test]
fn zero_empty_and_u64_max_maps_are_exact_and_do_not_reorder_source_votes() {
    for votes in [
        json!([]),
        json!([{"voteAccount":key(4),"stake":"0"}]),
        json!([{"voteAccount":key(4),"stake":u64::MAX.to_string()},{"voteAccount":key(2),"stake":"0"}]),
    ] {
        let mut value = fixture();
        let total = votes
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["stake"].as_str().unwrap().parse::<u64>().unwrap())
            .sum::<u64>();
        value["runtime"]["epochStakes"]["voteStakes"] = votes.clone();
        value["runtime"]["epochStakes"]["totalStake"] = json!(total.to_string());
        reseal(&mut value);
        assert_bound_context(&value).unwrap();
        assert_eq!(value["runtime"]["epochStakes"]["voteStakes"], votes);
    }
}

#[test]
fn source_order_proof_and_lineage_order_are_distinct_contracts() {
    let mut value = fixture();
    // New build receipts do not change the frozen runtime/feature identities.
    value["runtime"]["binding"]["bindingHash"] = json!(hash(b"new build binding"));
    value["runtime"]["binding"]["executor"]["sourceEvidenceHash"] =
        json!(hash(b"new build source"));
    assert_bound_context(&value).unwrap();
    let snapshot = &value["runtime"]["epochStakes"];
    let sorted_lineage = json!({"sourceHash":snapshot["blockEvidenceSha256"],"targetSlot":snapshot["slot"],
        "parentSlot":snapshot["parentSlot"],"blockhash":snapshot["blockhash"],
        "previousBlockhash":value["runtime"]["bankContext"]["executionBlockhash"]});
    value["runtime"]["bankContext"]["blockLineageHash"] =
        json!(hash(canonical_json(&sorted_lineage)));
    assert!(assert_epoch_stake_binding(&value, None).is_err());
    assert!(assert_initialized_stake_binding(&value, None).is_err());
}

#[test]
fn epoch_failure_is_typed_context_bound_and_never_a_certified_result() {
    let mut fixture = fixture();
    fixture["runtime"]
        .as_object_mut()
        .unwrap()
        .remove("epochStakes");
    let failure = json!({"schema":"svm-epoch-stake-input-failure/v1","status":"UNSUPPORTED",
        "code":"UNSUPPORTED_HISTORICAL_EPOCH_STAKE","reason":"complete current-epoch snapshot required",
        "guardedExecutionAttempted":true,"certifiedResult":false,"targetSlot":20,"epoch":"4","read":{"kind":"total"}});
    validate_epoch_stake_failure(&failure, &fixture).unwrap();
    let mut vote = failure.clone();
    vote["read"] = json!({"kind":"vote","voteAccount":key(2)});
    validate_epoch_stake_failure(&vote, &fixture).unwrap();
    for (field, changed) in [
        ("certifiedResult", json!(true)),
        ("guardedExecutionAttempted", json!(false)),
        ("epoch", json!("01")),
        ("epoch", json!("5")),
        ("targetSlot", json!(1.1)),
        ("targetSlot", json!(21)),
        ("output", json!({})),
        ("reason", json!("secret endpoint")),
        ("read", Value::Null),
        ("read", json!({"kind":"total","voteAccount":key(2)})),
        ("read", json!({"kind":"unknown"})),
        ("read", json!({"kind":"vote","voteAccount":"invalid"})),
    ] {
        let mut changed_failure = failure.clone();
        changed_failure[field] = changed;
        assert!(
            validate_epoch_stake_failure(&changed_failure, &fixture).is_err(),
            "{field}"
        );
    }
    fixture["runtime"]["epochStakes"] = Value::Null;
    assert!(validate_epoch_stake_failure(&failure, &fixture).is_err());
}

#[test]
fn discovery_response_requires_complete_shapes_and_proven_progress() {
    let fixture = discovery_fixture();
    let available = available_generic_sysvars(&fixture).unwrap();
    let rent = "SysvarRent111111111111111111111111111111111";
    let read = json!({"pubkey":rent,"offset":"0","length":"8"});
    let needs = json!({"schema":"svm-sysvar-discovery/v1","status":"NEEDS_INPUT","pubkey":rent,"reads":[read]});
    assert_eq!(
        parse_sysvar_discovery_response(&needs, &available).unwrap(),
        needs
    );
    for mutation in [
        "schema",
        "observation",
        "unknown-id",
        "already-available",
        "output-leak",
        "numeric-u64",
        "overflow",
        "leading-zero",
        "pubkey",
        "extra-read",
    ] {
        let mut value = needs.clone();
        match mutation {
            "schema" => value["schema"] = json!("other"),
            "observation" => value["reads"] = json!([]),
            "unknown-id" => {
                value["pubkey"] = json!(key(1));
                value["reads"][0]["pubkey"] = json!(key(1));
            }
            "already-available" => {
                value["pubkey"] = fixture["clock"]["pubkey"].clone();
                value["reads"][0]["pubkey"] = value["pubkey"].clone();
            }
            "output-leak" => value["output"] = json!({"partial":true}),
            "numeric-u64" => value["reads"][0]["offset"] = json!(0),
            "overflow" => value["reads"][0]["length"] = json!("18446744073709551616"),
            "leading-zero" => value["reads"][0]["offset"] = json!("00"),
            "pubkey" => value["reads"][0]["pubkey"] = json!("bad"),
            "extra-read" => value["reads"][0]["extra"] = json!(false),
            _ => unreachable!(),
        }
        assert!(
            parse_sysvar_discovery_response(&value, &available).is_err(),
            "{mutation}"
        );
    }
    let complete = json!({"schema":"svm-sysvar-discovery/v1","status":"COMPLETE","output":{},
        "reads":[{"pubkey":fixture["clock"]["pubkey"],"offset":"0","length":"8"}]});
    assert_eq!(
        parse_sysvar_discovery_response(&complete, &available).unwrap(),
        complete
    );
    let mut unproven = complete.clone();
    unproven["reads"][0]["pubkey"] = json!(rent);
    assert!(parse_sysvar_discovery_response(&unproven, &available).is_err());
    // Unknown (non-cache) IDs remain real observations, not demanded cache inputs.
    unproven["reads"][0]["pubkey"] = json!(key(7));
    parse_sysvar_discovery_response(&unproven, &available).unwrap();
}
