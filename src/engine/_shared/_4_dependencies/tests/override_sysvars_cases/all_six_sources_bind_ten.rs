use super::*;

#[test]
fn all_six_sources_bind_ten_exact_images_to_their_actual_phases() {
    let features = json!([]);
    let mut keys = vec!["SysvarC1ock11111111111111111111111111111111"];
    keys.extend(BANK_INITIALIZED_SYSVARS);
    keys.push(SLOT_HASHES);
    keys.extend(LEGACY);
    keys.push("11111111111111111111111111111111");
    for source in EXECUTORS {
        let c = OverrideSysvarContext {
            executor_source_id: source,
            ..context(&features)
        };
        let mut reads = vec![];
        let output =
            hydrate_override_sysvar_baselines_with_accounts(&edits(&keys), &[], &c, |key, slot| {
                reads.push((key.to_owned(), slot));
                Ok(vec![account(key, slot)])
            })
            .unwrap();
        assert_eq!(reads.len(), 10);
        assert_eq!(output.accounts.len(), 10);
        assert_eq!(output.bindings.len(), 10);
        assert_eq!(output.generic_sysvars.len(), 1);
        assert_eq!(output.generic_sysvars[0]["pubkey"], SLOT_HASHES);
        for a in &output.accounts {
            let key = a["pubkey"].as_str().unwrap();
            let slot = if key == SLOT_HASHES { 100 } else { 97 };
            assert_eq!(a, &account(key, slot));
            let b = output.bindings.iter().find(|b| b["pubkey"] == key).unwrap();
            assert_eq!(b["sourceSlot"], slot);
            assert_eq!(b["executorSourceId"], source);
            assert_eq!(
                b["phase"],
                if key == SLOT_HASHES {
                    "target-initialized"
                } else if LEGACY.contains(&key) {
                    "parent-end/pre-target"
                } else {
                    "parent-input-for-bank-initialization"
                }
            );
        }
    }
}

#[test]
fn cached_parent_legacy_is_unchanged_but_slot_hashes_is_replaced_not_relabeled() {
    let features = json!([]);
    let c = context(&features);
    let input = vec![account(LEGACY[0], 97), account(SLOT_HASHES, 97)];
    let before = input.clone();
    let mut reads = vec![];
    let output = hydrate_override_sysvar_baselines_with_accounts(
        &edits(&[LEGACY[0], SLOT_HASHES]),
        &input,
        &c,
        |key, slot| {
            reads.push((key.to_owned(), slot));
            Ok(vec![account(key, slot)])
        },
    )
    .unwrap();
    assert_eq!(reads, vec![(SLOT_HASHES.into(), 100)]);
    assert_eq!(input, before);
    assert_eq!(output.accounts[1], account(SLOT_HASHES, 100));
    let cached = hydrate_override_sysvar_baselines_with_accounts(
        &edits(&[LEGACY[0], SLOT_HASHES]),
        &output.accounts,
        &c,
        |_, _| panic!("cached fetch"),
    )
    .unwrap();
    assert_eq!(cached.generic_sysvars, output.generic_sysvars);
    assert!(bind_exact_generic_sysvar(&output.accounts[0], 100).is_err());
}

#[test]
fn missing_wrong_phase_or_invalid_full_image_never_gets_overridden_into_a_baseline() {
    let features = json!([]);
    let c = context(&features);
    for records in [
        vec![],
        vec![json!({"pubkey":LEGACY[0],"sourceSlot":97,"role":"application","presence":"absent"})],
        vec![account(LEGACY[1], 97)],
        vec![account(LEGACY[0], 97), account(LEGACY[0], 97)],
    ] {
        let error = hydrate_override_sysvar_baselines_with_accounts(
            &edits(&[LEGACY[0]]),
            &[],
            &c,
            |_, _| Ok(records.clone()),
        )
        .unwrap_err();
        assert_eq!(error.code, "UNSUPPORTED_BANK_INPUT");
        assert_eq!(error.details.unwrap()["phase"], "RECONSTRUCTION");
    }
    for patch in [
        json!({"sourceSlot":100}),
        json!({"owner":"11111111111111111111111111111111"}),
        json!({"executable":true}),
        json!({"lamports":"0"}),
        json!({"lamports":"18446744073709551616"}),
        json!({"lamports":"01"}),
        json!({"rentEpoch":"0"}),
        json!({"role":"application"}),
        json!({"dataBase64":"not base64"}),
        json!({"dataBase64":null}),
    ] {
        let mut a = account(LEGACY[0], 97);
        for (k, v) in patch.as_object().unwrap() {
            a[k] = v.clone();
        }
        let input = vec![a];
        let before = input.clone();
        let e = hydrate_override_sysvar_baselines_with_accounts(
            &edits(&[LEGACY[0]]),
            &input,
            &c,
            |_, _| panic!("invalid existing must not fetch"),
        )
        .unwrap_err();
        assert_eq!(e.code, "UNSUPPORTED_BANK_INPUT");
        assert_eq!(input, before);
    }
}

#[test]
fn proof_reuse_binds_all_metadata_phase_source_and_image_fields() {
    let features = json!([]);
    let c = context(&features);
    let output =
        hydrate_override_sysvar_baselines_with_accounts(&edits(&LEGACY), &[], &c, |key, slot| {
            Ok(vec![account(key, slot)])
        })
        .unwrap();
    let proven = proven_parent_override_sysvars(&output.accounts, &output.bindings, &c).unwrap();
    assert_eq!(proven.len(), 4);
    for field in [
        "accountSha256",
        "dataSha256",
        "bankSourceCommit",
        "executorSourceId",
        "policy",
        "sourceSlot",
        "parentSlot",
        "targetSlot",
        "pubkey",
    ] {
        let mut b = output.bindings.clone();
        b[0][field] = if field.ends_with("Slot") {
            json!(12)
        } else {
            json!("wrong")
        };
        assert!(
            proven_parent_override_sysvars(&output.accounts, &b, &c).is_err(),
            "{field}"
        );
    }
    for field in ["lamports", "dataBase64", "rentEpoch", "owner"] {
        let mut a = output.accounts.clone();
        a[0][field] = json!("wrong");
        assert!(proven_parent_override_sysvars(&a, &output.bindings, &c).is_err());
    }
    let mut duplicated = output.bindings.clone();
    duplicated.push(duplicated[0].clone());
    assert!(proven_parent_override_sysvars(&output.accounts, &duplicated, &c).is_err());
    let mut a = output.accounts.clone();
    a.push(a[0].clone());
    assert!(proven_parent_override_sysvars(&a, &output.bindings, &c).is_err());
    assert!(proven_parent_override_sysvars(&output.accounts, &[], &c)
        .unwrap()
        .is_empty());
    for features in [
        Value::Null,
        json!([1]),
        json!(["a1penGLz8Vm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP"]),
    ] {
        let changed = context(&features);
        assert_eq!(
            proven_parent_override_sysvars(&output.accounts, &output.bindings, &changed)
                .unwrap_err()
                .code,
            "UNSUPPORTED_RUNTIME_PROFILE"
        );
    }
}
