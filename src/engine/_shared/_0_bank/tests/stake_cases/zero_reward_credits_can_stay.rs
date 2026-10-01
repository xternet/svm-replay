use super::*;

#[test]
fn zero_reward_credits_can_stay_advance_or_rewind_but_metadata_cannot_disappear() {
    for credit in [0u64, 1, 2] {
        let mut source = input(true);
        change_bytes(&mut source["targetAccount"], |b| {
            b[188..196].copy_from_slice(&credit.to_le_bytes())
        });
        assert_eq!(
            recover_untouched_reward_stake(&source).unwrap()["proof"]["rewardLamports"],
            "0"
        );
        assert_eq!(
            prepare_stake_initialization(&prepared(&source)).unwrap()["accounts"],
            json!([strict(&source["targetAccount"], 20)])
        );
    }
    for mutation in ["lamports", "delegation", "owner-fields", "late-access"] {
        let mut source = input(true);
        match mutation {
            "lamports" => change(&mut source["targetAccount"], |v| {
                v["result"]["value"]["lamports"] = json!(1)
            }),
            "delegation" => change_bytes(&mut source["targetAccount"], |b| b[156] ^= 1),
            "owner-fields" => change_bytes(&mut source["targetAccount"], |b| b[20] ^= 1),
            "late-access" => change(&mut source["block"], |v| {
                v["result"]["transactions"][0] = instruction("payer")
            }),
            _ => unreachable!(),
        }
        assert!(
            recover_untouched_reward_stake(&source).is_err(),
            "{mutation}"
        );
    }
    for missing in [true, false] {
        let mut source = input(true);
        change(&mut source["block"], |v| {
            if missing {
                v["result"].as_object_mut().unwrap().remove("rewards");
            } else {
                v["result"]["rewards"] = Value::Null;
            }
        });
        assert!(recover_untouched_reward_stake(&source).is_err());
    }
}

#[test]
fn preparation_requires_exact_complete_scope_and_preserves_nonstake_inputs() {
    let source = input(false);
    let base = prepared(&source);
    let result = prepare_stake_initialization(&base).unwrap();
    assert_eq!(
        result["accounts"],
        json!([strict(&source["targetAccount"], 20)])
    );
    for mutation in [
        "no-evidence",
        "parent-lamports",
        "parent-bytes",
        "parent-slot",
        "block-hash",
        "envelope",
        "duplicate",
        "missing",
    ] {
        let mut value = base.clone();
        match mutation {
            "no-evidence" => {
                value.as_object_mut().unwrap().remove("evidence");
            }
            "parent-lamports" => value["accounts"][0]["lamports"] = json!("1"),
            "parent-bytes" => value["accounts"][0]["dataBase64"] = json!(STANDARD.encode([0; 200])),
            "parent-slot" => value["accounts"][0]["sourceSlot"] = json!(20),
            "block-hash" => value["blockSourceHash"] = json!(hash(b"wrong")),
            "envelope" => value["envelope"]["result"]["blockHeight"] = json!(11),
            "duplicate" => {
                let row = value["evidence"]["accounts"][0].clone();
                value["evidence"]["accounts"]
                    .as_array_mut()
                    .unwrap()
                    .push(row);
            }
            "missing" => value["evidence"]["accounts"] = json!([]),
            _ => unreachable!(),
        }
        assert!(prepare_stake_initialization(&value).is_err(), "{mutation}");
    }
    let mut value = base;
    value["accounts"][0]["owner"] = json!(key(9));
    assert!(prepare_stake_initialization(&value).is_err());
    value.as_object_mut().unwrap().remove("evidence");
    assert_eq!(
        prepare_stake_initialization(&value).unwrap()["accounts"],
        value["accounts"]
    );
}

#[test]
fn rent_adjustment_requires_exact_phase_and_handles_reduction_emptying_and_unchanged_zero() {
    for zero in [false, true] {
        for emptied in [false, true] {
            let mut source = adjusted(zero, if emptied { 100 } else { 20 }, 1.0);
            let expected = if emptied {
                0
            } else if zero {
                3440
            } else {
                3447
            };
            change_bytes(&mut source["targetAccount"], |b| {
                b[156..164].copy_from_slice(&(expected as u64).to_le_bytes());
                if emptied {
                    b[172..180].copy_from_slice(&3u64.to_le_bytes());
                }
            });
            let result = recover_untouched_reward_stake(&source).unwrap();
            assert!(result["proof"]["phasePolicy"]
                .as_str()
                .unwrap()
                .contains("rent-adjusted"));
            let mut off = source.clone();
            off["runtime"]["activeExecutionFeatureIds"] = json!([]);
            assert!(recover_untouched_reward_stake(&off).is_err());
            for mutation in [
                "absent-context",
                "clock-slot",
                "rent-absent",
                "rent-owner",
                "clock-key",
                "rent-size",
                "clock-bytes",
                "rent-rate",
            ] {
                let mut value = source.clone();
                match mutation {
                    "absent-context" => {
                        value.as_object_mut().unwrap().remove("phaseContext");
                    }
                    "clock-slot" => value["phaseContext"]["clock"]["sourceSlot"] = json!(19),
                    "rent-absent" => value["phaseContext"]["rent"]["presence"] = json!("absent"),
                    "rent-owner" => value["phaseContext"]["rent"]["owner"] = json!(key(5)),
                    "clock-key" => value["phaseContext"]["clock"]["pubkey"] = json!(key(5)),
                    "rent-size" => value["phaseContext"]["rent"]["dataBase64"] = json!("AA=="),
                    "clock-bytes" => {
                        value["phaseContext"]["clock"]["dataBase64"] =
                            json!(STANDARD.encode([0u8; 40]))
                    }
                    "rent-rate" => {
                        let mut rent = STANDARD
                            .decode(
                                value["phaseContext"]["rent"]["dataBase64"]
                                    .as_str()
                                    .unwrap(),
                            )
                            .unwrap();
                        rent[..8].copy_from_slice(&21u64.to_le_bytes());
                        value["phaseContext"]["rent"]["dataBase64"] = json!(STANDARD.encode(rent));
                    }
                    _ => unreachable!(),
                }
                // A different rate that still empties delegation is valid; use nonempty cases for that mutation.
                if mutation != "rent-rate" || !emptied {
                    assert!(
                        recover_untouched_reward_stake(&value).is_err(),
                        "{mutation}"
                    );
                }
            }
        }
    }
    let mut unchanged = adjusted(true, 100, 1.0);
    let old = bytes(&unchanged["parentAccount"]);
    change(&mut unchanged["targetAccount"], |v| {
        v["result"]["value"]["data"][0] = json!(STANDARD.encode(old))
    });
    recover_untouched_reward_stake(&unchanged).unwrap();
    for field in ["parentAccount", "targetAccount"] {
        change_bytes(&mut unchanged[field], |b| b[156..164].fill(0));
    }
    recover_untouched_reward_stake(&unchanged).unwrap();
}
