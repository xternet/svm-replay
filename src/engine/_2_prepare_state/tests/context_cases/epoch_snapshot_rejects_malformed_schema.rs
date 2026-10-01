use super::*;

#[test]
fn epoch_snapshot_rejects_malformed_schema_integrity_lineage_clock_and_complete_map() {
    for mutation in [
        "unknown-field",
        "null",
        "schema",
        "proof",
        "source-body",
        "source-kind",
        "lineage",
        "clock-bytes",
        "clock-owner",
        "epoch",
        "total",
        "duplicate-vote",
        "overflow",
        "incomplete",
        "runtime",
        "genesis",
    ] {
        let mut value = fixture();
        match mutation {
            "unknown-field" => value["runtime"]["epochStakes"]["extra"] = json!(true),
            "null" => value["runtime"]["epochStakes"] = Value::Null,
            "schema" => value["runtime"]["epochStakes"]["schema"] = json!("unknown"),
            "proof" => value["runtime"]["epochStakes"]["proofHash"] = json!(hash(b"wrong")),
            "source-body" => {
                value["runtime"]["epochStakes"]["source"]["evidenceSha256"] = json!(hash(b"wrong"))
            }
            "source-kind" => value["runtime"]["epochStakes"]["source"]["kind"] = json!("untrusted"),
            "lineage" => value["runtime"]["bankContext"]["executionBlockhash"] = json!(key(7)),
            "clock-bytes" => value["clock"]["dataBase64"] = json!(STANDARD.encode([0u8; 40])),
            "clock-owner" => value["clock"]["owner"] = json!(key(7)),
            "epoch" => {
                value["runtime"]["epochStakes"]["epoch"] = json!("5");
                reseal(&mut value);
            }
            "total" => {
                value["runtime"]["epochStakes"]["totalStake"] = json!("8");
                reseal(&mut value);
            }
            "duplicate-vote" => {
                let copy = value["runtime"]["epochStakes"]["voteStakes"][0].clone();
                value["runtime"]["epochStakes"]["voteStakes"]
                    .as_array_mut()
                    .unwrap()
                    .push(copy);
                reseal(&mut value);
            }
            "overflow" => {
                value["runtime"]["epochStakes"]["voteStakes"] = json!([{"voteAccount":key(2),"stake":u64::MAX.to_string()},{"voteAccount":key(4),"stake":"1"}]);
                reseal(&mut value);
            }
            "incomplete" => {
                value["runtime"]["epochStakes"]["complete"] = json!(false);
                reseal(&mut value);
            }
            "runtime" => value["runtime"]["binding"]["runtimeProfileId"] = json!("different"),
            "genesis" => {
                value["runtime"]["epochStakes"]["genesisHash"] = json!("malformed");
                reseal(&mut value);
            }
            _ => unreachable!(),
        }
        assert!(
            assert_epoch_stake_binding(&value, None).is_err(),
            "{mutation}"
        );
    }
    assert!(assert_epoch_stake_binding(&fixture(), Some(&key(5))).is_err());
}

#[test]
fn initialized_snapshot_requires_exact_complete_images_and_consistent_genesis() {
    for mutation in [
        "schema",
        "phase",
        "unknown-field",
        "proof",
        "source",
        "lineage",
        "parent-image",
        "initialized-image",
        "duplicate",
        "missing-stake",
        "extra-stake",
        "mixed-genesis",
        "lamports",
        "bytes",
    ] {
        let mut value = fixture();
        match mutation {
            "schema" => {
                value["runtime"]["initializedStakeSnapshot"]["schema"] = json!("unknown");
                reseal(&mut value);
            }
            "phase" => {
                value["runtime"]["initializedStakeSnapshot"]["phase"] = json!("end-of-slot");
                reseal(&mut value);
            }
            "unknown-field" => value["runtime"]["initializedStakeSnapshot"]["extra"] = json!(true),
            "proof" => {
                value["runtime"]["initializedStakeSnapshot"]["proofHash"] = json!(hash(b"wrong"))
            }
            "source" => {
                value["runtime"]["initializedStakeSnapshot"]["source"]["kind"] = json!("untrusted")
            }
            "lineage" => {
                value["runtime"]["bankContext"]["blockLineageHash"] = json!(hash(b"wrong"))
            }
            "parent-image" => {
                value["runtime"]["initializedStakeSnapshot"]["accounts"][0]["parent"]
                    ["sourceSlot"] = json!(20);
                reseal(&mut value);
            }
            "initialized-image" => value["accounts"][0]["dataBase64"] = json!("Aw=="),
            "duplicate" => {
                let copy = value["runtime"]["initializedStakeSnapshot"]["accounts"][0].clone();
                value["runtime"]["initializedStakeSnapshot"]["accounts"]
                    .as_array_mut()
                    .unwrap()
                    .push(copy);
                reseal(&mut value);
            }
            "missing-stake" => value["accounts"] = json!([]),
            "extra-stake" => {
                let mut copy = value["accounts"][0].clone();
                copy["pubkey"] = json!(key(5));
                value["accounts"].as_array_mut().unwrap().push(copy);
            }
            "mixed-genesis" => {
                value["runtime"]["initializedStakeSnapshot"]["genesisHash"] = json!(key(5));
                reseal(&mut value);
            }
            "lamports" => {
                value["runtime"]["initializedStakeSnapshot"]["accounts"][0]["parent"]["lamports"] =
                    json!("18446744073709551616");
                reseal(&mut value);
            }
            "bytes" => {
                value["runtime"]["initializedStakeSnapshot"]["accounts"][0]["parent"]
                    ["dataBase64"] = json!("%%% ");
                reseal(&mut value);
            }
            _ => unreachable!(),
        }
        assert!(
            assert_initialized_stake_binding(&value, None).is_err(),
            "{mutation}"
        );
    }
    assert!(assert_initialized_stake_binding(&fixture(), Some(&key(5))).is_err());
}

#[test]
fn absent_context_is_optional_but_null_and_unguarded_workers_are_rejected() {
    let mut value = fixture();
    value["runtime"]["binding"]["executor"]["m9Build"]["capabilities"] = json!([]);
    assert!(assert_bound_context(&value).is_err());
    value["runtime"]
        .as_object_mut()
        .unwrap()
        .remove("epochStakes");
    value["runtime"]
        .as_object_mut()
        .unwrap()
        .remove("initializedStakeSnapshot");
    assert_bound_context(&value).unwrap();
    for field in ["epochStakes", "initializedStakeSnapshot"] {
        let mut changed = value.clone();
        changed["runtime"][field] = Value::Null;
        assert!(assert_bound_context(&changed).is_err(), "{field}");
    }
}
