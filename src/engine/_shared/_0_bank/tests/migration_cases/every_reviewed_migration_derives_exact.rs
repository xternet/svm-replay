use super::*;

#[test]
fn every_reviewed_migration_derives_exact_pointer_images_proof_and_parent_check() {
    for index in 0..4 {
        for authority in [false, true] {
            let input = input(index, authority);
            let before = input.clone();
            let context = program_migration_context(&input).unwrap();
            assert_eq!(input, before);
            assert_eq!(context["proofs"].as_array().unwrap().len(), 1);
            assert_eq!(context["proofs"][0]["featureId"], ROWS[index].0);
            assert_eq!(context["proofs"][0]["buffer"], ROWS[index].3);
            let resolved =
                resolve_program_migration_overlays(&context, &[ROWS[index].1.into()], &json!({}))
                    .unwrap();
            assert_eq!(resolved[ROWS[index].1], ROWS[index].2);
            validate_program_migration_parents(&context, context["parents"].as_array().unwrap())
                .unwrap();
            assert_program_migration_bindings(&fixture(&input, &context)).unwrap();
            let mut basis = context["proofs"][0].clone();
            let observed = basis.as_object_mut().unwrap().remove("proofHash").unwrap();
            assert_eq!(
                observed,
                hash(format!(
                    "program-migration-initialization/v1\n{}\n",
                    canonical_json(&basis)
                ))
            );
        }
    }
}

#[test]
fn missing_unrelated_and_out_of_range_crossings_never_use_target_images() {
    let mut input = input(0, false);
    input.as_object_mut().unwrap().remove("evidence");
    assert!(program_migration_context(&input).is_err());
    for activation in [19, 21] {
        input["runtime"]["features"][0]["activationSlot"] = json!(activation);
        let context = program_migration_context(&input).unwrap();
        assert_eq!(context["proofs"], json!([]));
        let resolved = resolve_program_migration_overlays(
            &context,
            &[ROWS[0].1.into()],
            &json!({ROWS[0].1:Value::Null}),
        )
        .unwrap();
        assert_eq!(resolved[ROWS[0].1], Value::Null);
        assert!(
            resolve_program_migration_overlays(&context, &[ROWS[0].1.into()], &json!({})).is_err()
        );
    }
    let mut input = super::input(0, false);
    input["requestedPubkeys"] = json!([key(7)]);
    let context = program_migration_context(&input).unwrap();
    assert!(validate_program_migration_parents(&context, &[]).is_err());
}

#[test]
fn migration_rejects_mutated_phase_images_receipts_writers_and_runtime() {
    for mutation in [
        "slot",
        "bytes",
        "executor",
        "alpenglow",
        "authority",
        "program",
        "funding",
        "buffer",
        "source-buffer",
        "rent-size",
        "rent-threshold",
        "receipt",
        "set",
        "parent-slot",
        "writer",
        "reward",
        "partial-block",
    ] {
        let mut input = input(if mutation == "authority" { 2 } else { 0 }, true);
        match mutation {
            "slot" => input["slot"] = json!(21),
            "bytes" => change_bytes(
                &mut input["evidence"]["accounts"][1]["targetAccount"],
                |b| b[47] ^= 1,
            ),
            "executor" => input["runtime"]["executor"]["id"] = json!("wrong"),
            "alpenglow" => input["runtime"]["features"].as_array_mut().unwrap().push(
                json!({"id":"a1penGLz8Vm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP","activationSlot":1}),
            ),
            "authority" => change_bytes(
                &mut input["evidence"]["accounts"][2]["parentAccount"],
                |b| b[5] ^= 1,
            ),
            "program" => change_bytes(
                &mut input["evidence"]["accounts"][0]["targetAccount"],
                |b| b[4] ^= 1,
            ),
            "funding" => change(
                &mut input["evidence"]["accounts"][1]["targetAccount"],
                |v| v["result"]["value"]["lamports"] = json!(1),
            ),
            "buffer" => {
                let value = body(&input["evidence"]["accounts"][2]["parentAccount"])["result"]
                    ["value"]
                    .clone();
                change(
                    &mut input["evidence"]["accounts"][2]["targetAccount"],
                    |v| v["result"]["value"] = value,
                );
            }
            "source-buffer" => change_bytes(
                &mut input["evidence"]["accounts"][2]["parentAccount"],
                |b| b[0] = 9,
            ),
            "rent-size" => change_bytes(&mut input["evidence"]["rent"], |b| {
                b.pop();
            }),
            "rent-threshold" => change_bytes(&mut input["evidence"]["rent"], |b| {
                b[8..16].copy_from_slice(&1.5f64.to_le_bytes())
            }),
            "receipt" => {
                input["evidence"]["accounts"][0]["targetAccount"]["responseSha256"] =
                    json!(hash(b"wrong"))
            }
            "set" => {
                input["evidence"]["accounts"].as_array_mut().unwrap().pop();
            }
            "parent-slot" => {
                input["evidence"]["accounts"][0]["parentAccount"]["request"]["params"][1]["slot"] =
                    json!(18)
            }
            "partial-block" => {
                input["evidence"]["block"]["request"]["params"][1]["transactionDetails"] =
                    json!("none")
            }
            "writer" | "reward" => {
                change(&mut input["evidence"]["block"], |v| {
                    if mutation == "reward" {
                        v["result"]["rewards"] =
                            json!([{"pubkey":ROWS[0].3,"rewardType":"Fee","lamports":1}]);
                    } else {
                        v["result"]["transactions"] = json!([{"version":"legacy","transaction":{"signatures":[bs58::encode([7u8;64]).into_string()],
                        "message":{"accountKeys":[ROWS[0].3,"11111111111111111111111111111111"],"header":{"numRequiredSignatures":1,"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":1},"instructions":[]}},"meta":{"err":null,"innerInstructions":[],"loadedAddresses":{"writable":[],"readonly":[]}}}]);
                    }
                });
                input["envelope"] = body(&input["evidence"]["block"]);
                input["blockSourceHash"] = input["evidence"]["block"]["responseSha256"].clone();
            }
            _ => unreachable!(),
        }
        assert!(program_migration_context(&input).is_err(), "{mutation}");
    }
}
