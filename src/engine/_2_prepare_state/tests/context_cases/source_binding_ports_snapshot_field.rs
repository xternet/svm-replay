use super::*;

#[test]
fn source_binding_ports_snapshot_field_mutations_after_independent_resealing() {
    let fixture = fixture();
    let epoch_changes = [
        ("/snapshot/genesisHash", json!(key(7))),
        ("/snapshot/slot", json!(21)),
        ("/snapshot/parentSlot", json!(18)),
        ("/snapshot/blockhash", json!(key(7))),
        ("/snapshot/blockEvidenceSha256", json!(hash(b"other"))),
        ("/snapshot/executorSourceId", json!("wrong")),
        ("/snapshot/runtimeProfileId", json!("wrong")),
        ("/snapshot/activeFeatureSetHash", json!(hash(b"other"))),
        ("/snapshot/clockDataSha256", json!(hash(b"other"))),
        ("/snapshot/epoch", json!("5")),
        ("/snapshot/totalStake", json!("8")),
        ("/snapshot/totalStake", json!("07")),
        ("/snapshot/totalStake", json!("18446744073709551616")),
        ("/snapshot/voteStakes/0/stake", json!(7)),
        ("/snapshot/voteStakes/0/stake", json!("-1")),
        (
            "/snapshot/voteStakes/0/stake",
            json!("18446744073709551616"),
        ),
        ("/snapshot/voteStakes/0/voteAccount", json!("invalid")),
        (
            "/snapshot/voteStakes",
            json!([{"voteAccount":key(2),"stake":"7"},{"voteAccount":key(2),"stake":"0"}]),
        ),
        (
            "/snapshot/voteStakes",
            json!([{"voteAccount":key(2),"stake":u64::MAX.to_string()},{"voteAccount":key(3),"stake":"1"}]),
        ),
        ("/snapshot/complete", json!(false)),
        ("/snapshot/voteStakes", Value::Null),
        ("/source/kind", json!("guessed-balances")),
        ("/source/id", json!("https://private.invalid/key")),
    ];
    for (path, value) in epoch_changes {
        let mut evidence = evidence(&fixture["runtime"]["epochStakes"]);
        *evidence.pointer_mut(path).unwrap() = value;
        evidence["source"]["evidenceSha256"] = json!(hash(canonical_json(&evidence["snapshot"])));
        assert!(
            bind_epoch_stake_evidence(&json!({"fixture":fixture,"evidence":evidence})).is_err(),
            "epoch {path}"
        );
    }
    let initialized_changes = [
        ("/snapshot/accounts", json!([])),
        ("/snapshot/accounts/0/parent/pubkey", json!(key(7))),
        ("/snapshot/slot", json!(21)),
        ("/snapshot/parentSlot", json!(18)),
        ("/snapshot/blockhash", json!(key(7))),
        ("/snapshot/blockEvidenceSha256", json!(hash(b"other"))),
        ("/snapshot/executorSourceId", json!("wrong")),
        ("/snapshot/runtimeProfileId", json!("wrong")),
        ("/snapshot/activeFeatureSetHash", json!(hash(b"other"))),
        ("/snapshot/genesisHash", json!(key(7))),
        ("/snapshot/phase", json!("post-transaction")),
        ("/snapshot/accounts/0/parent/lamports", json!("1")),
        ("/snapshot/accounts/0/initialized/sourceSlot", json!(19)),
        ("/snapshot/accounts/0/initialized/owner", json!(key(7))),
        ("/snapshot/accounts/0/initialized/presence", json!("absent")),
        ("/snapshot/accounts/0/initialized/executable", json!(true)),
        ("/snapshot/accounts/0/initialized/lamports", json!(9)),
        ("/snapshot/accounts/0/initialized/lamports", json!("09")),
        (
            "/snapshot/accounts/0/initialized/rentEpoch",
            json!("18446744073709551616"),
        ),
        ("/snapshot/accounts/0/initialized/dataBase64", json!("AA")),
        ("/source/kind", json!("inferred-end-state")),
        ("/source/id", json!("https://private.invalid/key")),
    ];
    for (path, value) in initialized_changes {
        let mut input = initialized_request(&fixture);
        *input["evidence"].pointer_mut(path).unwrap() = value;
        input["evidence"]["source"]["evidenceSha256"] =
            json!(hash(canonical_json(&input["evidence"]["snapshot"])));
        assert!(
            prepare_exact_initialized_stakes(&input).is_err(),
            "initialized {path}"
        );
    }
}

#[test]
fn source_evidence_binders_reject_untrusted_incomplete_and_mixed_context() {
    for mutation in [
        "shape",
        "genesis",
        "source",
        "body",
        "slot",
        "parent",
        "block",
        "runtime",
        "parent-image",
        "missing",
        "duplicate",
    ] {
        let mut input = initialized_request(&fixture());
        match mutation {
            "shape" => input["evidence"]["extra"] = json!(true),
            "genesis" => input["evidence"]["expectedGenesisHash"] = json!(key(7)),
            "source" => input["evidence"]["source"]["kind"] = json!("current-state"),
            "body" => {
                input["evidence"]["snapshot"]["accounts"][0]["initialized"]["dataBase64"] =
                    json!("Aw==")
            }
            "slot" => input["slot"] = json!(21),
            "parent" => input["parentSlot"] = json!(18),
            "block" => input["blockhash"] = json!(key(7)),
            "runtime" => input["runtime"]["executor"]["id"] = json!("different"),
            "parent-image" => input["accounts"][0]["lamports"] = json!("99"),
            "missing" => input["accounts"] = json!([]),
            "duplicate" => {
                let row = input["accounts"][0].clone();
                input["accounts"].as_array_mut().unwrap().push(row);
            }
            _ => unreachable!(),
        }
        assert!(
            prepare_exact_initialized_stakes(&input).is_err(),
            "{mutation}"
        );
    }
    for mutation in ["shape", "genesis", "source", "body", "clock", "lineage"] {
        let fixture = fixture();
        let mut input =
            json!({"evidence":evidence(&fixture["runtime"]["epochStakes"]),"fixture":fixture});
        match mutation {
            "shape" => input["evidence"]["extra"] = json!(true),
            "genesis" => input["evidence"]["expectedGenesisHash"] = json!(key(7)),
            "source" => input["evidence"]["source"]["kind"] = json!("current-state"),
            "body" => input["evidence"]["snapshot"]["totalStake"] = json!("8"),
            "clock" => input["fixture"]["clock"]["dataBase64"] = json!("AA=="),
            "lineage" => {
                input["fixture"]["runtime"]["bankContext"]["executionBlockhash"] = json!(key(7))
            }
            _ => unreachable!(),
        }
        assert!(bind_epoch_stake_evidence(&input).is_err(), "{mutation}");
    }
}
