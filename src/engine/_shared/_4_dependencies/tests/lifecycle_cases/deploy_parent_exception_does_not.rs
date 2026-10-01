use super::*;

#[test]
fn deploy_parent_exception_does_not_relax_unrelated_roles_or_install_state() {
    let records = vec![absent(1), absent(2)];
    let before = records.clone();
    let mut roles = Roles::default();
    roles.programs.insert(key(1));
    roles.program_data.insert(key(2));
    let evidence = vec![IncludedTransaction {
        index: 0,
        raw: raw(&[2], false),
    }];
    let result =
        build_inspected_loader_parent_accounts(&records, 100, &roles, &evidence, 1).unwrap();
    assert_eq!(result[0]["role"], "program");
    assert_eq!(result[1]["role"], "programdata");
    assert_eq!(result[0]["presence"], "absent");
    assert_eq!(records, before);
    assert!(build_inspected_loader_parent_accounts(&records, 100, &roles, &[], 1).is_err());
    assert!(build_inspected_loader_parent_accounts(
        &records,
        100,
        &roles,
        &[IncludedTransaction {
            index: 2,
            raw: raw(&[2], false)
        }],
        1
    )
    .is_err());
    for role in ["sysvar", "alt", "excluded"] {
        let mut changed = roles.clone();
        match role {
            "sysvar" => {
                changed.sysvars.insert(key(1));
            }
            "alt" => {
                changed.address_tables.insert(key(1));
            }
            _ => {
                changed.excluded.insert(key(1));
            }
        }
        assert!(
            build_inspected_loader_parent_accounts(&records, 100, &changed, &evidence, 1).is_err()
        );
    }
}

#[test]
fn alt_instruction_host_admission_covers_exact_finite_forms_without_runtime_shortcuts() {
    let alt = "AddressLookupTab1e1111111111111111111111111";
    for inner in [false, true] {
        for tag in 0u32..=5 {
            let mut value = raw(&[0], inner);
            value["transaction"]["message"]["accountKeys"][8] = json!(alt);
            let mut data = vec![
                0;
                match tag {
                    0 => 13,
                    2 => 44,
                    _ => 4,
                }
            ];
            data[..4].copy_from_slice(&tag.to_le_bytes());
            if tag == 2 {
                data[4..12].copy_from_slice(&1u64.to_le_bytes());
            }
            let instruction = if inner {
                &mut value["meta"]["innerInstructions"][0]["instructions"][0]
            } else {
                &mut value["transaction"]["message"]["instructions"][0]
            };
            instruction["data"] = json!(bs58::encode(data).into_string());
            instruction["accounts"] = json!([0, 1, 2, 3]);
            let summary = summarize_semantic_transaction(&value, 0).unwrap();
            let result = assert_program_lifecycle_supported(&[summary], Some(&[value]), None);
            if tag < 5 {
                assert_eq!(result.unwrap(), vec![0]);
            } else {
                assert_eq!(result.unwrap_err().code, "UNSUPPORTED_ALT_LIFECYCLE");
            }
        }
    }
}

#[test]
#[ignore = "explicit development differential requires Bun and SVM_REPLAY_PROTOTYPE"]
fn original_loader_differential_for_temporal_and_parent_state_matrix() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut cases = Vec::new();
    let mut actual = Vec::new();
    for tag in 0..=8 {
        for inner in [false, true] {
            for state in 0..7 {
                for evidence in 0..6 {
                    for failed in [false, true] {
                        let mut c = context();
                        match state {
                            0 => {}
                            1 => {
                                c.accounts[0] = absent(1);
                                c.accounts[1] = absent(2);
                            }
                            2 => c.accounts[2] = absent(3),
                            3 => c.accounts[2] = account(3, 0),
                            4 => {
                                c.accounts.remove(1);
                            }
                            5 => c.accounts[2]["sourceSlot"] = json!(99),
                            6 => c.accounts[0]["dataBase64"] = json!(STANDARD.encode([0; 36])),
                            _ => unreachable!(),
                        }
                        let mut value = raw(&[tag], inner);
                        c.included_transactions = match evidence {
                            0 => vec![],
                            1 => vec![IncludedTransaction {
                                index: 0,
                                raw: raw(&[0], inner),
                            }],
                            2 => vec![IncludedTransaction {
                                index: 0,
                                raw: raw(&[2], inner),
                            }],
                            3 => {
                                value = raw(&[0, 2, tag], inner);
                                vec![IncludedTransaction {
                                    index: 1,
                                    raw: value.clone(),
                                }]
                            }
                            4 => vec![IncludedTransaction {
                                index: 2,
                                raw: raw(&[0, 2], inner),
                            }],
                            5 => vec![IncludedTransaction {
                                index: 1,
                                raw: raw(&[0, 2], inner),
                            }],
                            _ => unreachable!(),
                        };
                        if failed {
                            value["meta"]["err"] =
                                json!({"InstructionError":[0,"IncorrectAuthority"]});
                            if evidence == 3 {
                                c.included_transactions[0].raw = value.clone();
                            }
                        }
                        actual.push(match inspect(&value, &c) {
                            Ok(()) => json!({"ok":true}),
                            Err(e) => json!({"error":e.message}),
                        });
                        cases.push(json!({"raw":value,"index":1,"context":c,"case":[tag,inner,state,evidence,failed]}));
                    }
                }
            }
        }
    }
    let script = r#"const {assertInspectedLoaderLifecycle}=await import(process.env.SVM_REPLAY_PROTOTYPE+'/poc/m9-program-lifecycle.ts');const results=[];for(const c of await Bun.stdin.json()){try{assertInspectedLoaderLifecycle(c.raw,c.index,c.context);results.push({ok:true});}catch(e){results.push({error:e.message});}}console.log(JSON.stringify(results));"#;
    let mut child = Command::new("bun")
        .args(["--eval", script])
        .env(
            "SVM_REPLAY_PROTOTYPE",
            std::env::var("SVM_REPLAY_PROTOTYPE").expect("explicit prototype path"),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("original loader");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&cases).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = svm_replay_protocol::parse_json(&output.stdout).unwrap();
    for (i, (a, b)) in actual.iter().zip(expected.as_array().unwrap()).enumerate() {
        assert_eq!(a, b, "case {}", cases[i]["case"]);
    }
    assert_eq!(actual.len(), 9 * 2 * 7 * 6 * 2);
    assert_eq!(actual.len(), expected.as_array().unwrap().len());
}
