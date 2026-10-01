use super::*;

#[test]
fn all_reviewed_eras_and_nonmutating_accesses_preserve_zero_and_positive_images() {
    let eras = [
        "litesvm-v0.7.1-agave-2.3.9",
        "litesvm-v0.8.2-agave-3.0.10",
        "litesvm-v0.12.0-agave-3.1.11",
        "litesvm-v0.13.1-agave-4.0.0",
        "litesvm-v0.14.0-pr402-agave-4.1.2",
        "litesvm-v0.16.0-agave-4.2.1",
    ];
    for era in eras {
        for kind in ["untouched", "readonly", "loaded-readonly", "failed-writer"] {
            for zero in [false, true] {
                let mut source = input(zero);
                source["runtime"]["executorSourceId"] = json!(era);
                change(&mut source["block"], |v| {
                    v["result"]["transactions"][0] = instruction(kind)
                });
                let before = source.clone();
                let result = recover_untouched_reward_stake(&source).unwrap();
                assert_eq!(result["account"], strict(&source["targetAccount"], 20));
                assert_eq!(source, before);
                if kind == "untouched" {
                    assert!(result["proof"].get("transactionAccessProof").is_none());
                    assert_eq!(
                        result["proof"]["schema"],
                        "svm-simulate-untouched-stake-initialization/v1"
                    );
                } else {
                    assert_eq!(
                        result["proof"]["transactionAccessProof"][0]["kind"],
                        if kind == "failed-writer" {
                            "failed-nonpersistent"
                        } else {
                            "readonly"
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn persistent_writes_and_malformed_failures_never_authorize_recovery() {
    for kind in ["success-writer", "loaded-writer", "payer", "nonce"] {
        let mut source = input(false);
        change(&mut source["block"], |v| {
            v["result"]["transactions"][0] = instruction(kind)
        });
        assert!(recover_untouched_reward_stake(&source).is_err(), "{kind}");
    }
    for error in [
        json!(false),
        json!({}),
        json!({"InstructionError":[99999,"InvalidArgument"]}),
        json!({"InstructionError":[0,null]}),
        json!({"InstructionError":[0,{"Custom":4294967296u64}]}),
        json!({"InstructionError":[0,{"Custom":0,"Extra":1}]}),
    ] {
        let mut source = input(false);
        change(&mut source["block"], |v| {
            v["result"]["transactions"][0] = instruction("failed-writer");
            v["result"]["transactions"][0]["meta"]["err"] = error;
        });
        assert!(recover_untouched_reward_stake(&source).is_err());
    }
    let mut source = input(false);
    change(&mut source["block"], |v| {
        v["result"]["transactions"][0] = instruction("failed-writer");
        v["result"]["transactions"][0]["meta"]["err"] =
            json!({"InstructionError":[0,{"Custom":u32::MAX}]});
    });
    recover_untouched_reward_stake(&source).unwrap();
}

#[test]
fn corrupted_receipts_boundaries_rewards_and_stake_dimensions_reject() {
    for mutation in [
        "slot",
        "request",
        "account",
        "parent",
        "height",
        "hash",
        "missing-tx",
        "truncated",
        "missing-loaded",
        "not-full",
        "fee-reward",
        "duplicate-reward",
        "era",
        "alpenglow",
        "owner",
        "executable",
        "rent",
        "lamports",
        "data",
        "delegation",
        "credits",
        "size",
        "version",
        "lookup-account",
        "duplicate-tx",
    ] {
        let mut source = input(false);
        match mutation {
            "slot" => change(&mut source["targetAccount"], |v| {
                v["result"]["context"]["slot"] = json!(19)
            }),
            "request" => source["targetAccount"]["request"]["params"][1]["slot"] = json!(19),
            "account" => source["targetAccount"]["request"]["params"][0] = json!(key(5)),
            "parent" => change(&mut source["block"], |v| {
                v["result"]["previousBlockhash"] = json!(key(6))
            }),
            "height" => source["boundary"]["blockHeight"] = json!(11),
            "hash" => source["targetAccount"]["responseSha256"] = json!(hash(b"wrong")),
            "missing-tx" => change(&mut source["block"], |v| {
                v["result"].as_object_mut().unwrap().remove("transactions");
            }),
            "truncated" => change(&mut source["block"], |v| {
                v["result"]["transactions"] = json!([])
            }),
            "missing-loaded" => change(&mut source["block"], |v| {
                v["result"]["transactions"][0] = instruction("loaded-readonly");
                v["result"]["transactions"][0]["meta"]
                    .as_object_mut()
                    .unwrap()
                    .remove("loadedAddresses");
            }),
            "not-full" => {
                source["block"]["request"]["params"][1]["transactionDetails"] = json!("none")
            }
            "fee-reward" => change(&mut source["block"], |v| {
                v["result"]["rewards"][0]["rewardType"] = json!("Fee")
            }),
            "duplicate-reward" => change(&mut source["block"], |v| {
                let row = v["result"]["rewards"][0].clone();
                v["result"]["rewards"].as_array_mut().unwrap().push(row);
            }),
            "era" => source["runtime"]["executorSourceId"] = json!("unknown"),
            "alpenglow" => {
                source["runtime"]["activeExecutionFeatureIds"] =
                    json!(["a1penGLz8Vm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP"])
            }
            "owner" => change(&mut source["targetAccount"], |v| {
                v["result"]["value"]["owner"] = json!(key(5))
            }),
            "executable" => change(&mut source["targetAccount"], |v| {
                v["result"]["value"]["executable"] = json!(true)
            }),
            "rent" => change(&mut source["targetAccount"], |v| {
                v["result"]["value"]["rentEpoch"] = json!(0)
            }),
            "lamports" => change(&mut source["targetAccount"], |v| {
                v["result"]["value"]["lamports"] = json!(1)
            }),
            "data" => change_bytes(&mut source["targetAccount"], |b| b[20] ^= 1),
            "delegation" => change_bytes(&mut source["targetAccount"], |b| b[156] ^= 1),
            "credits" => change_bytes(&mut source["targetAccount"], |b| b[188..196].fill(0)),
            "size" => change_bytes(&mut source["targetAccount"], |b| {
                b.pop();
            }),
            "version" => change(&mut source["block"], |v| {
                v["result"]["transactions"][0]["version"] = json!(1)
            }),
            "lookup-account" => change(&mut source["block"], |v| {
                v["result"]["transactions"][0] = instruction("loaded-readonly");
                v["result"]["transactions"][0]["transaction"]["message"]["addressTableLookups"]
                    [0]["accountKey"] = json!(key(1));
            }),
            "duplicate-tx" => {
                change(&mut source["block"], |v| {
                    let row = v["result"]["transactions"][0].clone();
                    v["result"]["transactions"]
                        .as_array_mut()
                        .unwrap()
                        .push(row);
                });
                source["boundary"]["transactionCount"] = json!(2);
            }
            _ => unreachable!(),
        }
        assert!(
            recover_untouched_reward_stake(&source).is_err(),
            "{mutation}"
        );
    }
}
