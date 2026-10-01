use super::*;

#[test]
fn override_membership_accepts_related_backing_and_known_sysvars_not_unrelated_accounts() {
    let accounts = programs();
    let edits=validate_requested_overrides(&json!([{"pubkey":key(82),"lamports":"5000000"},{"pubkey":OVERRIDABLE_SYSVARS[0],"lamports":"2"}])).expect("edits");
    assert!(assert_override_membership(&edits, &[key(81)], &[], &accounts).is_ok());
    assert!(
        assert_override_membership(&edits, &[key(83)], &[], &accounts)
            .expect_err("unrelated")
            .message
            .contains("UNRELATED_ACCOUNT_OVERRIDE")
    );
    let runtime = validate_requested_overrides(
        &json!([{"pubkey":"Sysvar1nstructions1111111111111111111111111","lamports":"1"}]),
    )
    .expect("shape");
    assert!(assert_override_membership(&runtime, &[], &[], &accounts)
        .expect_err("runtime derived")
        .message
        .contains("RUNTIME_DERIVED_SYSVAR_OVERRIDE"));
}

#[test]
fn program_image_inspection_is_hypothetical_and_rejects_metadata_relationship_edits() {
    let accounts = programs();
    let before = accounts.clone();
    let mut bytes = STANDARD
        .decode(accounts[1]["dataBase64"].as_str().expect("data"))
        .expect("decode");
    bytes.truncate(45);
    bytes.extend(b"\x7fELFsol_get_sysvar");
    let edits = validate_requested_overrides(
        &json!([{"pubkey":key(82),"dataBase64":STANDARD.encode(&bytes)}]),
    )
    .expect("edit");
    let inspected = requested_image_inspection(&accounts, &edits).expect("inspection");
    assert_eq!(accounts, before);
    assert_ne!(inspected[1]["dataBase64"], accounts[1]["dataBase64"]);
    bytes[12] = 2;
    let edits = validate_requested_overrides(
        &json!([{"pubkey":key(82),"dataBase64":STANDARD.encode(bytes)}]),
    )
    .expect("edit");
    assert!(requested_image_inspection(&accounts, &edits).is_err());
    let edits = validate_requested_overrides(
        &json!([{"pubkey":key(81),"dataBase64":STANDARD.encode([0_u8;36])}]),
    )
    .expect("edit");
    assert!(requested_image_inspection(&accounts, &edits)
        .expect_err("relationship")
        .message
        .contains("ProgramData relationship"));
    let mut sysvar = account(
        "Sysvar1111111111111111111111111111111111111",
        &STANDARD.encode([0_u8; 40]),
    );
    sysvar["pubkey"] = json!(OVERRIDABLE_SYSVARS[0]);
    sysvar["role"] = json!("sysvar");
    let edits = validate_requested_overrides(
        &json!([{"pubkey":OVERRIDABLE_SYSVARS[0],"dataBase64":"AQ=="}]),
    )
    .expect("edit");
    assert_eq!(
        requested_image_inspection(&[sysvar.clone()], &edits).expect("sysvar remains historical"),
        vec![sysvar]
    );
}

#[test]
fn hypothetical_deactivation_requires_its_own_slot_hashes_evidence() {
    let mut bytes = STANDARD.decode(table(4, 99, 0)).expect("table");
    for (deactivation, expected) in [(99_u64, true), (100, false), (u64::MAX, false)] {
        bytes[4..12].copy_from_slice(&deactivation.to_le_bytes());
        assert_eq!(
            overridden_lookup_needs_slot_hashes(&edit(&STANDARD.encode(&bytes)), &[key(3)], 100)
                .expect("deactivation"),
            expected
        );
    }
    assert!(
        !overridden_lookup_needs_slot_hashes(&edit(&STANDARD.encode(&bytes)), &[], 100)
            .expect("unrelated")
    );
}

#[test]
fn created_parent_role_changes_do_not_install_override_bytes_or_mutate_originals() {
    for inner in [false, true] {
        for parent in [absent(), account(&key(0), "")] {
            let records = vec![parent.clone()];
            let roles = roles();
            let raws = vec![mutation(0, false, inner)];
            let selected = summaries(&raws);
            let edits = edit(&table(4, 99, 0));
            let result = prepare_created_alt_parents(CreatedAltInput {
                records: &records,
                roles: &roles,
                parent_slot: 99,
                target_slot: 100,
                target_index: 1,
                selected: &selected,
                raw_transactions: &raws,
                requested_table_keys: &[key(3)],
                overrides: &edits,
            })
            .expect("created parent proof");
            assert!(!result.parent_roles.address_tables.contains(&key(3)));
            assert!(roles.address_tables.contains(&key(3)));
            assert_eq!(records, vec![parent.clone()]);
            assert_eq!(
                serde_json::to_value(&result.created_tables).expect("proof"),
                json!([{"pubkey":key(3),"parent":if parent["presence"]=="absent"{"absent"}else{"prefunded-system"},"parentSlot":99,"targetSlot":100,"targetIndex":1,"selectedWriterIndices":[0]}])
            );
        }
    }
}

#[test]
fn created_parent_rejects_unselected_failed_future_ambiguous_and_conflicting_roles() {
    let raws = vec![mutation(0, false, false)];
    let selected = summaries(&raws);
    let records = vec![absent()];
    let edits = edit(&table(4, 99, 0));
    for selected in [
        vec![],
        {
            let mut s = selected.clone();
            s[0].index = 1;
            s
        },
        {
            let mut s = selected.clone();
            s.extend(s.clone());
            s
        },
        summaries(&[mutation(0, true, true)]),
    ] {
        assert!(prepare_created_alt_parents(CreatedAltInput {
            records: &records,
            roles: &roles(),
            parent_slot: 99,
            target_slot: 100,
            target_index: 1,
            selected: &selected,
            raw_transactions: &raws,
            requested_table_keys: &[key(3)],
            overrides: &edits
        })
        .is_err());
    }
    let mut conflicted = roles();
    conflicted.sysvars.insert(key(3));
    assert!(prepare_created_alt_parents(CreatedAltInput {
        records: &records,
        roles: &conflicted,
        parent_slot: 99,
        target_slot: 100,
        target_index: 1,
        selected: &selected,
        raw_transactions: &raws,
        requested_table_keys: &[key(3)],
        overrides: &edits
    })
    .is_err());
}
