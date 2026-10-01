use super::*;

#[test]
fn same_slot_extend_freeze_and_deactivate_preserve_only_existing_address_prefix() {
    for inner in [false, true] {
        for tag in [1, 2, 3] {
            let raw = mutation(tag, false, inner);
            let resolved = resolve(
                &original(false),
                &[raw.clone()],
                account(ALT, &table(4, 99, 0)),
                &[],
                0,
            )
            .expect("unchanged prefix");
            assert!(resolved.summary.declared_accounts.contains(&key(4)));
            assert!(resolve_requested_dependencies_with_accounts(
                &wire(true, 0),
                &original(false),
                &summaries(&[raw]),
                1,
                99,
                |_, _| panic!("missing boundary"),
                None,
                &[]
            )
            .expect_err("must not borrow parent across unknown lifecycle")
            .message
            .contains("same-slot table boundary"));
        }
    }
}

#[test]
fn committed_outer_and_cpi_create_allow_explicit_absent_or_prefunded_parent_only() {
    for inner in [false, true] {
        for parent in [absent(), account(&key(0), "")] {
            let raw = mutation(0, false, inner);
            let resolved = resolve(&original(true), &[raw], parent, &edit(&table(4, 99, 0)), 0)
                .expect("created boundary table");
            assert!(resolved.summary.declared_accounts.contains(&key(4)));
        }
    }
    for mut parent in [
        account(&key(8), ""),
        account(&key(0), &STANDARD.encode([0_u8; 80])),
        account(&key(0), ""),
    ] {
        if parent["owner"] == key(0) && parent["dataBase64"] == "" {
            parent["executable"] = json!(true);
        }
        assert!(resolve(
            &original(true),
            &[mutation(0, false, false)],
            parent,
            &edit(&table(4, 99, 0)),
            0
        )
        .expect_err("invalid prefunding")
        .message
        .contains("malformed replacement lookup table"));
    }
}

#[test]
fn failed_create_cannot_make_absence_present_and_committed_close_cannot_be_overridden() {
    for inner in [false, true] {
        assert_eq!(
            resolve(
                &original(true),
                &[mutation(0, true, inner)],
                absent(),
                &edit(&table(4, 99, 0)),
                0
            )
            .expect_err("failed create")
            .code,
            "UNSUPPORTED_ALT_LIFECYCLE"
        );
        assert!(resolve(
            &original(true),
            &[mutation(4, false, inner)],
            account(ALT, &table(2, 99, 0)),
            &edit(&table(4, 99, 0)),
            0
        )
        .expect_err("closed")
        .message
        .contains("INVALID_REQUESTED_LOOKUP_CLOSED"));
        assert!(resolve(
            &original(true),
            &[mutation(4, true, inner)],
            account(ALT, &table(2, 99, 0)),
            &edit(&table(4, 99, 0)),
            0
        )
        .is_ok());
    }
    assert!(resolve(
        &original(false),
        &[mutation(0, false, false)],
        absent(),
        &[],
        0
    )
    .expect_err("created table has no old index")
    .message
    .contains("INVALID_REQUESTED_LOOKUP_INDEX"));
}

#[test]
fn writer_identity_closed_recreation_and_complete_cpi_metadata_are_required() {
    let raw = mutation(0, false, false);
    let writers = summaries(&[raw.clone()]);
    assert!(assert_requested_parent_alt_still_usable(
        &key(3),
        &writers,
        99,
        &boundary(vec![mutation(4, false, false)])
    )
    .expect_err("identity")
    .message
    .contains("identity mismatch"));
    assert!(resolve(
        &original(true),
        &[mutation(4, false, false), raw.clone()],
        account(ALT, &table(2, 99, 0)),
        &edit(&table(4, 99, 0)),
        0
    )
    .expect_err("recreated")
    .message
    .contains("INVALID_REQUESTED_LOOKUP_RECREATED"));
    for inner in [
        Value::Null,
        json!([{"index":1,"instructions":[]}]),
        json!([{"index":0,"instructions":[]},{"index":0,"instructions":[]}]),
    ] {
        let mut modified = raw.clone();
        modified["meta"]["innerInstructions"] = inner;
        let summaries = summaries(&[modified.clone()]);
        assert!(assert_requested_parent_alt_still_usable(
            &key(3),
            &summaries,
            99,
            &boundary(vec![modified])
        )
        .is_err());
    }
    let malformed = mutation(99, false, false);
    let error = assert_requested_parent_alt_still_usable(
        &key(3),
        &summaries(&[malformed.clone()]),
        99,
        &boundary(vec![malformed]),
    )
    .expect_err("unknown mutation");
    assert_eq!(error.code, "UNSUPPORTED_ALT_LIFECYCLE");
    let future = RequestedBoundaryEvidence {
        target_slot: 99,
        raw_transactions: vec![raw],
    };
    assert!(assert_requested_parent_alt_still_usable(&key(3), &writers, 99, &future).is_err());
}

#[test]
fn failed_writer_still_needs_cpi_recording_but_not_decoding_rolled_back_instruction_data() {
    let mut raw = mutation(4, true, false);
    raw["transaction"]["message"]["instructions"][0]["data"] = Value::Null;
    let writers = summaries(&[raw.clone()]);
    assert!(
        !assert_requested_parent_alt_still_usable(&key(3), &writers, 99, &boundary(vec![raw]))
            .expect("failed mutation rolls back after metadata validation")
    );
}

#[test]
fn requested_alt_invocation_is_guarded_by_declared_accounts() {
    assert!(requested_cannot_invoke_alt(
        &summarize_semantic_transaction(&original(true), 0).expect("summary")
    ));
    assert!(!requested_cannot_invoke_alt(
        &summarize_semantic_transaction(&mutation(0, false, true), 0).expect("summary")
    ));
}
