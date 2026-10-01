use super::*;

#[test]
fn prefunded_token_data_requires_relax_feature_and_bindings_require_exact_images() {
    let mut input = input(0, false);
    change(
        &mut input["evidence"]["accounts"][1]["parentAccount"],
        |v| v["result"]["value"] = image("11111111111111111111111111111111", false, 1, &[]),
    );
    assert!(program_migration_context(&input).is_err());
    input["runtime"]["features"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"rexav5eNTUSNT1K2N7cfRjnthwhcP5BC25v2tA4rW4h","activationSlot":1}));
    let context = program_migration_context(&input).unwrap();
    let base = fixture(&input, &context);
    for mutation in ["rent", "block", "omitted", "parent", "target", "proof"] {
        if mutation == "parent" {
            let mut parents = context["parents"].as_array().unwrap().clone();
            parents[0]["lamports"] = json!("99");
            assert!(validate_program_migration_parents(&context, &parents).is_err());
            continue;
        }
        let mut fixture = base.clone();
        match mutation {
            "rent" => {
                fixture["accounts"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()["lamports"] = json!("2")
            }
            "block" => fixture["runtime"]["bankContext"]["blockSourceHash"] = json!(hash(b"wrong")),
            "omitted" => {
                let proof = &mut fixture["runtime"]["programMigrationProofs"][0];
                proof["targetAccountHashes"].as_array_mut().unwrap().pop();
                let mut basis = proof.clone();
                basis.as_object_mut().unwrap().remove("proofHash");
                proof["proofHash"] = json!(hash(format!(
                    "program-migration-initialization/v1\n{}\n",
                    canonical_json(&basis)
                )));
            }
            "target" => fixture["accounts"][0]["sourceSlot"] = json!(19),
            "proof" => {
                fixture["runtime"]["programMigrationProofs"][0]["proofHash"] = json!(hash(b"wrong"))
            }
            _ => unreachable!(),
        }
        assert!(
            assert_program_migration_bindings(&fixture).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn common_preflight_rejects_mutated_migration_images_before_prepared_or_discovery_execution() {
    let input = input(0, false);
    let context = program_migration_context(&input).unwrap();
    let mut fixture = fixture(&input, &context);
    svm_replay_engine::_2_prepare_state::context::assert_bound_context(&fixture).unwrap();
    fixture["accounts"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["lamports"] = json!("2");
    let error =
        svm_replay_engine::_2_prepare_state::context::assert_bound_context(&fixture).unwrap_err();
    assert_eq!(error.code, "UNSUPPORTED_BANK_INPUT");
    assert!(error.message.contains("initialized Rent changed"));
}

#[test]
#[ignore = "requires explicit SVM_REPLAY_TEST_MIGRATION_ARTIFACT_ROOT preserved Token/Stake captures and original runtime fixtures"]
fn preserved_token_and_two_stake_activation_captures_recompute_exact_images() {
    let root = std::path::PathBuf::from(
        std::env::var("SVM_REPLAY_TEST_MIGRATION_ARTIFACT_ROOT")
            .expect("SVM_REPLAY_TEST_MIGRATION_ARTIFACT_ROOT"),
    );
    let read = |path: std::path::PathBuf| -> Value {
        svm_replay_protocol::parse_json(&std::fs::read(path).unwrap()).unwrap()
    };
    for (capture, offset, profile) in [
        ("m9-token-activation-9pQCB5", 0, 14),
        ("m9-stake-activation-2Ydf8n", 0, 21),
        ("m9-stake-activation-2Ydf8n", 9, 32),
    ] {
        let receipt = |n: usize| {
            read(
                root.join(capture)
                    .join(format!("rpc-{:03}.json", n + offset)),
            )
        };
        let block = receipt(1);
        let envelope = body(&block);
        let slot = block["request"]["params"][0].clone();
        let parent = envelope["result"]["parentSlot"].clone();
        let historical = read(root.join(format!(
            "m8-hard-parity-2026-09-04/cases/m7-profile-{profile}/attempt-9/fixture.json"
        )));
        let accounts=[2,4,6].into_iter().map(|n|{let parent=receipt(n);json!({"pubkey":parent["request"]["params"][0],"parentAccount":parent,"targetAccount":receipt(n+1)})}).collect::<Vec<_>>();
        let program = accounts[0]["pubkey"].as_str().unwrap().to_owned();
        let pd = accounts[1]["pubkey"].as_str().unwrap().to_owned();
        let input = json!({"slot":slot,"parentSlot":parent,"blockSourceHash":block["responseSha256"],"envelope":envelope,
            "runtime":historical["runtime"]["binding"],"requestedPubkeys":[program],"evidence":{"evidenceKind":"provider-attested","block":block,"accounts":accounts,"rent":receipt(8)}});
        let context = program_migration_context(&input).unwrap();
        assert_eq!(context["proofs"].as_array().unwrap().len(), 1);
        assert_eq!(context["proofs"][0]["evidenceKind"], "provider-attested");
        let links = resolve_program_migration_overlays(
            &context,
            std::slice::from_ref(&program),
            &json!({}),
        )
        .unwrap();
        assert_eq!(links[&program], pd);
        for (index, row) in input["evidence"]["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let value = body(&row["targetAccount"])["result"]["value"].clone();
            let role = match index {
                0 => "program",
                1 => "programdata",
                _ => "application",
            };
            let mut expected = json!({"pubkey":row["pubkey"],"sourceSlot":slot,"role":role,"presence":if value.is_null(){"absent"}else{"present"}});
            if !value.is_null() {
                for name in ["lamports", "rentEpoch"] {
                    expected[name] =
                        json!(svm_replay_engine::shared::diff::exact_u64(&value[name])
                            .unwrap()
                            .to_string());
                }
                expected["owner"] = value["owner"].clone();
                expected["executable"] = value["executable"].clone();
                expected["dataBase64"] = value["data"][0].clone();
            }
            assert_eq!(
                context["initialized"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["pubkey"] == row["pubkey"])
                    .unwrap(),
                &expected
            );
        }
        validate_program_migration_parents(&context, context["parents"].as_array().unwrap())
            .unwrap();
        assert_program_migration_bindings(&fixture(&input, &context)).unwrap();
        let mut expanded = input.clone();
        expanded["requestedPubkeys"] = json!(input["evidence"]["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["pubkey"].clone())
            .collect::<Vec<_>>());
        assert_eq!(program_migration_context(&expanded).unwrap(), context);
    }
}
