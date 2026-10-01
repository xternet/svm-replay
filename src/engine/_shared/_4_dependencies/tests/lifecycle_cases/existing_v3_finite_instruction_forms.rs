use super::*;

#[test]
fn existing_v3_finite_instruction_forms_and_failed_authority_remain_runtime_inputs() {
    for inner in [false, true] {
        for tag in 0..=7 {
            let mut value = raw(&[tag], inner);
            value["meta"]["err"] = json!({"InstructionError":[0,"IncorrectAuthority"]});
            assert!(
                inspect(&value, &context()).is_ok(),
                "tag {tag}, inner {inner}"
            );
        }
    }
    let mut c = context();
    let mut bytes = STANDARD
        .decode(c.accounts[1]["dataBase64"].as_str().unwrap())
        .unwrap();
    bytes[12] = 0;
    c.accounts[1]["dataBase64"] = json!(STANDARD.encode(bytes));
    assert!(inspect(&raw(&[6], false), &c).is_ok());
}

#[test]
fn loader_parent_shape_pointer_slot_future_and_authority_guards() {
    for mode in 0..9 {
        let mut c = context();
        match mode {
            0 => {
                c.accounts.remove(0);
            }
            1 => {
                c.accounts.remove(1);
            }
            2 => {
                c.accounts.remove(2);
            }
            3 => c.accounts[1]["sourceSlot"] = json!(99),
            4 => c.accounts[2]["owner"] = json!(key(9)),
            5 => c.accounts[0]["dataBase64"] = json!(STANDARD.encode([0; 36])),
            6 | 7 => {
                let mut bytes = STANDARD
                    .decode(c.accounts[1]["dataBase64"].as_str().unwrap())
                    .unwrap();
                if mode == 6 {
                    bytes[4..12].copy_from_slice(&101u64.to_le_bytes());
                } else {
                    bytes[12] = 2;
                }
                c.accounts[1]["dataBase64"] = json!(STANDARD.encode(bytes));
            }
            8 => c.accounts.push(c.accounts[0].clone()),
            _ => unreachable!(),
        }
        assert!(inspect(&raw(&[3], false), &c).is_err(), "mode {mode}");
    }
}

#[test]
fn malformed_layouts_and_v4_are_explicit_preflight_unsupported() {
    for tag in 0..=8 {
        let mut value = raw(&[tag], false);
        value["transaction"]["message"]["instructions"][0]["data"] =
            json!(bs58::encode([tag as u8; 12]).into_string());
        let summary = summarize_semantic_transaction(&value, 0).unwrap();
        let error =
            assert_program_lifecycle_supported(&[summary], Some(&[value]), Some(&context()))
                .unwrap_err();
        assert_eq!(error.code, "UNSUPPORTED_PROGRAM_LIFECYCLE");
        assert_eq!(error.details.unwrap()["phase"], "PREFLIGHT");
    }
    let mut value = raw(&[3], false);
    value["transaction"]["message"]["accountKeys"][8] = json!(V4);
    let summary = summarize_semantic_transaction(&value, 0).unwrap();
    assert!(assert_program_lifecycle_supported(
        &[summary.clone()],
        Some(&[value]),
        Some(&context())
    )
    .unwrap_err()
    .message
    .contains("loader-v4"));
    assert!(assert_program_lifecycle_supported(&[summary], None, None)
        .unwrap_err()
        .message
        .contains("context missing"));
}

#[test]
fn exact_absent_programs_require_earlier_included_deploy_but_not_its_success() {
    for inner in [false, true] {
        for tag in [3, 5, 6, 7] {
            let mut c = context();
            c.accounts[0] = absent(1);
            c.accounts[1] = absent(2);
            let mut before = raw(&[2], inner);
            before["meta"]["err"] = json!({"InstructionError":[0,"IncorrectAuthority"]});
            c.included_transactions = vec![IncludedTransaction {
                index: 0,
                raw: before,
            }];
            let mut current = raw(&[tag], false);
            if tag == 7 {
                current["transaction"]["message"]["instructions"][0]["accounts"] = json!([1, 3, 4]);
            }
            let unchanged = c.accounts.clone();
            inspect(&current, &c).unwrap();
            assert_eq!(unchanged, c.accounts);
            c.included_transactions[0].index = 2;
            assert!(inspect(&current, &c).is_err());
        }
    }
}

#[test]
fn same_transaction_order_and_payload_identity_are_bound_for_outer_and_cpi() {
    for inner in [false, true] {
        let value = raw(&[2, 5], inner);
        let mut c = context();
        c.accounts[0] = absent(1);
        c.accounts[1] = absent(2);
        c.included_transactions = vec![IncludedTransaction {
            index: 1,
            raw: value.clone(),
        }];
        inspect(&value, &c).unwrap();
        let reversed = raw(&[5, 2], inner);
        c.included_transactions[0].raw = reversed.clone();
        assert!(inspect(&reversed, &c).is_err());
        assert!(inspect(&value, &c).is_err());
    }
}

#[test]
fn absent_or_uninitialized_buffer_needs_exact_earlier_initialization() {
    for state in [absent(3), account(3, 0)] {
        for tag in [1, 2, 4, 5, 7] {
            let mut value = raw(&[0, tag], false);
            if tag == 5 {
                value["transaction"]["message"]["instructions"][1]["accounts"] = json!([2, 4, 3]);
            }
            let mut c = context();
            c.accounts[2] = state.clone();
            c.included_transactions = vec![IncludedTransaction {
                index: 1,
                raw: value.clone(),
            }];
            inspect(&value, &c).unwrap();
            c.included_transactions.clear();
            // Closing an exact uninitialized loader account is a runtime path.
            assert_eq!(
                inspect(&value, &c).is_ok(),
                tag == 5 && state["presence"] == "present"
            );
        }
    }
}
