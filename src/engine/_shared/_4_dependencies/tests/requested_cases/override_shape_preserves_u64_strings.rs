use super::*;

#[test]
fn override_shape_preserves_u64_strings_and_rejects_invalid_fields_data_and_duplicates() {
    for lamports in ["0", "9007199254740993", "18446744073709551615"] {
        assert_eq!(
            serde_json::to_value(
                validate_requested_overrides(&json!([{"pubkey":"account","lamports":lamports}]))
                    .expect("u64 precision")
            )
            .expect("serialize"),
            json!([{"pubkey":"account","lamports":lamports}])
        );
    }
    for lamports in [
        json!("01"),
        json!("-1"),
        json!("1.0"),
        json!("1e3"),
        json!("18446744073709551616"),
        json!(9007199254740992_u64),
        Value::Null,
    ] {
        assert!(
            validate_requested_overrides(&json!([{"pubkey":"account","lamports":lamports}]))
                .is_err()
        );
    }
    for value in [
        json!([]),
        json!([{"pubkey":"account"}]),
        json!([{"pubkey":"account","lamports":"0"},{"pubkey":"account","dataBase64":""}]),
        json!([{"pubkey":"account","owner":"other","lamports":"1"}]),
        json!([{"pubkey":"account","dataBase64":"YQ"}]),
        json!([{"pubkey":"","lamports":"1"}]),
        json!([{"pubkey":"account","dataBase64":null}]),
    ] {
        assert!(validate_requested_overrides(&value).is_err(), "{value}");
    }
    let oversized = "A".repeat((10 * 1024 * 1024_usize).div_ceil(3) * 4 + 1);
    assert!(
        validate_requested_overrides(&json!([{"pubkey":"account","dataBase64":oversized}]))
            .is_err()
    );
}

#[test]
fn unchanged_lookup_uses_exact_archived_loaded_addresses_without_fetch() {
    let original = original(true);
    for edits in [
        vec![],
        validate_requested_overrides(&json!([{"pubkey":key(3),"lamports":"2000001"}]))
            .expect("lamport edit"),
    ] {
        let resolved = resolve_requested_dependencies_with_accounts(
            &wire(true, 0),
            &original,
            &[],
            0,
            99,
            |_, _| panic!("unchanged list must not refetch"),
            None,
            &edits,
        )
        .expect("archived list");
        assert!(resolved.summary.declared_accounts.contains(&key(2)));
        assert!(!resolved.summary.declared_accounts.contains(&key(4)));
    }
    let mut malformed = original;
    malformed["meta"]["loadedAddresses"]["writable"] = json!([]);
    assert!(resolve_requested_dependencies_with_accounts(
        &wire(true, 0),
        &malformed,
        &[],
        0,
        99,
        |_, _| panic!("must reject before fetch"),
        None,
        &[]
    )
    .expect_err("cardinality")
    .message
    .contains("cardinality"));
}

#[test]
fn replacement_only_table_fetch_is_exact_and_never_uses_tip_state() {
    let parent = account(ALT, &table(4, 99, 0));
    let resolved =
        resolve(&original(false), &[], parent.clone(), &[], 0).expect("replacement table");
    assert!(resolved.summary.declared_accounts.contains(&key(4)));
    let mut tip = parent;
    tip["sourceSlot"] = json!(100);
    assert_eq!(
        resolve(&original(false), &[], tip, &[], 0)
            .expect_err("tip")
            .code,
        "SOURCE_CONTEXT_MISMATCH"
    );
    let missing =
        resolve(&original(false), &[], absent(), &[], 0).expect_err("missing exact table");
    assert_eq!(missing.code, "UNSUPPORTED_ALT_LIFECYCLE");
}

#[test]
fn data_override_changes_unchanged_wire_dependencies_without_mutating_original() {
    let original = original(true);
    let before = original.clone();
    let resolved = resolve(
        &original,
        &[],
        account(ALT, &table(2, 99, 0)),
        &edit(&table(4, 99, 0)),
        0,
    )
    .expect("edited table");
    assert!(resolved.summary.declared_accounts.contains(&key(4)));
    assert!(!resolved.summary.declared_accounts.contains(&key(2)));
    assert_eq!(original, before);
    assert!(resolve_requested_dependencies_with_accounts(
        &wire(true, 0),
        &original,
        &[],
        0,
        99,
        |_, _| panic!("no boundary"),
        None,
        &edit(&table(4, 99, 0))
    )
    .expect_err("boundary required")
    .message
    .contains("exact target boundary"));
}

#[test]
fn current_slot_warmup_metadata_and_index_bounds_remain_strict() {
    let canonical = original(true);
    let parent = account(ALT, &table(2, 99, 0));
    assert!(
        resolve(&canonical, &[], parent.clone(), &edit(&table(4, 100, 0)), 0)
            .expect_err("cold index")
            .message
            .contains("INVALID_REQUESTED_LOOKUP_INDEX")
    );
    assert!(resolve(&canonical, &[], parent.clone(), &edit(&table(4, 100, 1)), 0).is_ok());
    for bytes in [
        vec![0_u8; 55],
        vec![0_u8; 88],
        {
            let mut b = STANDARD.decode(table(4, 99, 0)).expect("table");
            b[21] = 2;
            b
        },
        {
            let mut b = STANDARD.decode(table(4, 99, 0)).expect("table");
            b[20] = 2;
            b
        },
    ] {
        assert!(resolve(
            &canonical,
            &[],
            parent.clone(),
            &edit(&STANDARD.encode(bytes)),
            0
        )
        .expect_err("malformed table")
        .message
        .contains("invalid replacement lookup table state"));
    }
    assert!(resolve(&canonical, &[], parent.clone(), &edit(&table(4, 101, 0)), 0).is_err());
    assert!(resolve(&original(false), &[], parent, &[], 1)
        .expect_err("absent index")
        .message
        .contains("INVALID_REQUESTED_LOOKUP_INDEX"));
}
