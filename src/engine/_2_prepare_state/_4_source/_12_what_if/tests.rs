use super::*;
use dependencies::SemanticTransaction;
use svm_replay_protocol::{StateOverride, WhatIf, WhatIfMode};

fn request(what_if: WhatIf) -> HistoricalRequest {
    let mut value = json!({"schema":"svm-replay-historical/v1","requestId":"t","family":"v4-2",
        "genesisHash":"11111111111111111111111111111111","candidate":{},"runtimeBinding":{},
        "replacementTransactionBase64":null,"requestedAccountOverrides":null,"bankInputs":[],
        "metadataPolicy":"STRICT","limits":{"timeoutMs":1000,"maxOutputBytes":1024,"maxDiagnosticBytes":1024}});
    value["whatIf"] = serde_json::to_value(what_if).unwrap();
    serde_json::from_value(value).unwrap()
}

fn entry(pubkey: &str) -> StateOverride {
    StateOverride {
        pubkey: pubkey.into(),
        lamports: None,
        owner: None,
        executable: None,
        rent_epoch: None,
        data_base64: None,
    }
}

/// The historical Rent sysvar: 3480 lamports per byte-year, threshold 2.0, burn 50%.
fn accounts() -> Vec<Value> {
    let mut data = 3480_u64.to_le_bytes().to_vec();
    data.extend_from_slice(&2.0_f64.to_le_bytes());
    data.push(50);
    vec![json!({"pubkey":RENT,"presence":"present","dataBase64":STANDARD.encode(data)})]
}

#[test]
fn whole_account_overrides_reach_the_worker_with_every_field() {
    let program = StateOverride {
        owner: Some("BPFLoader2111111111111111111111111111111111".into()),
        executable: Some(true),
        data_base64: Some(STANDARD.encode([0x7f, b'E', b'L', b'F'])),
        ..entry("Arb1111111111111111111111111111111111111111")
    };
    let gone = StateOverride {
        lamports: Some("0".into()),
        ..entry("Gone111111111111111111111111111111111111111")
    };
    let patch = StateOverride {
        lamports: Some("5".into()),
        ..entry("Pay1111111111111111111111111111111111111111")
    };
    let input = request(WhatIf {
        mode: WhatIfMode::Replace,
        transactions_base64: None,
        state_overrides: vec![program, gone, patch.clone()],
    });
    let mut fixture = json!({});
    insert_variants(&mut fixture, &input, "TARGET", &accounts()).unwrap();
    let what_if = &fixture["whatIf"];
    assert_eq!(
        what_if["transactionsBase64"],
        json!(["TARGET"]),
        "re-runs the target"
    );
    let rows = what_if["stateOverrides"].as_array().unwrap();
    assert_eq!(
        rows[0]["lamports"],
        json!(((128 + 4) * 3480 * 2).to_string())
    );
    assert_eq!(rows[0]["rentEpoch"], json!(u64::MAX.to_string()));
    assert_eq!(rows[1]["owner"], json!("11111111111111111111111111111111"));
    assert_eq!(
        rows[2],
        serde_json::to_value(&patch).unwrap(),
        "patches stay partial"
    );
}

#[test]
fn inventory_full_replacement_does_not_retain_old_executable_or_programdata_bytes() {
    let key = "11111111111111111111111111111111";
    let owner = StateOverride {
        owner: Some(key.into()),
        ..entry(key)
    };
    let input = request(WhatIf {
        mode: WhatIfMode::Replace,
        transactions_base64: None,
        state_overrides: vec![owner],
    });
    let old = json!({"pubkey":key,"presence":"present","role":"programdata","executable":true,
        "owner":"BPFLoaderUpgradeab1e11111111111111111111111","dataBase64":STANDARD.encode([3,0,0,0,1])});
    let images = inventory_images(&[old], &[], &input).unwrap();
    assert_eq!(images[0]["executable"], false);
    assert_eq!(images[0]["dataBase64"], "");
    assert_eq!(images[0]["role"], "application");
}

#[test]
fn replaced_accounts_need_no_archive_image_unless_the_original_uses_them() {
    let pay = "Pay1111111111111111111111111111111111111111".to_owned();
    let original = SemanticTransaction {
        index: 0,
        signature: "s".into(),
        fee_payer: pay.clone(),
        declared_accounts: vec![pay.clone()],
        instruction_accounts: vec![],
        writable_accounts: vec![],
        application_writable_accounts: vec![],
        possible_persistent_writes: None,
        address_table_accounts: vec![],
        program_ids: vec![],
        program_data_accounts: vec![],
        semantic_dependency_accounts: vec![pay],
        excluded_dependency_accounts: vec![],
        fee_only_payer: false,
        succeeded: true,
        is_vote: false,
    };
    let mut bundle = original.clone();
    let mut canonical = request(WhatIf {
        mode: WhatIfMode::After,
        transactions_base64: None,
        state_overrides: vec![],
    });
    canonical.what_if = None;
    let mut unchanged = original.clone();
    unchanged
        .declared_accounts
        .push(unchanged.declared_accounts[0].clone());
    assert_eq!(
        dependency_target(&canonical, &unchanged, None)
            .unwrap()
            .declared_accounts,
        unchanged.declared_accounts,
        "no extra read scope must not rewrite existing transaction metadata"
    );
    bundle
        .declared_accounts
        .push("New1111111111111111111111111111111111111111".into());
    bundle
        .program_ids
        .push("New1111111111111111111111111111111111111111".into());
    let replaced = |key: &str| StateOverride {
        owner: Some("11111111111111111111111111111111".into()),
        ..entry(key)
    };
    let mut input = request(WhatIf {
        mode: WhatIfMode::After,
        transactions_base64: Some(vec!["AQ==".into()]),
        state_overrides: vec![
            replaced("New1111111111111111111111111111111111111111"),
            replaced("Pay1111111111111111111111111111111111111111"),
            StateOverride {
                lamports: Some("1".into()),
                ..entry("Pat1111111111111111111111111111111111111111")
            },
        ],
    });
    input.extra_accounts = vec![RENT.into()];
    let target = dependency_target(&input, &original, Some(&bundle)).unwrap();
    assert!(target.declared_accounts.iter().any(|key| key == RENT));
    assert!(target
        .semantic_dependency_accounts
        .iter()
        .any(|key| key == RENT));
    assert!(!target.writable_accounts.iter().any(|key| key == RENT));
    let deps = &target.semantic_dependency_accounts;
    assert!(
        !deps.iter().any(|k| k.starts_with("New")),
        "replaced and unused: not fetched"
    );
    assert!(
        target.program_ids.is_empty(),
        "a program an override places is not fetched as a program"
    );
    assert!(
        deps.iter().any(|k| k.starts_with("Pay")),
        "the original needs it"
    );
    assert!(
        deps.iter().any(|k| k.starts_with("Pat")),
        "patches need the current account"
    );
}

#[test]
fn a_bundle_after_the_target_cannot_use_a_lookup_table_the_target_changes() {
    let table = "Tab1111111111111111111111111111111111111111".to_owned();
    let original = SemanticTransaction {
        index: 0,
        signature: "s".into(),
        fee_payer: "Pay1111111111111111111111111111111111111111".into(),
        declared_accounts: vec![table.clone()],
        instruction_accounts: vec![],
        writable_accounts: vec![table.clone()],
        application_writable_accounts: vec![table.clone()],
        possible_persistent_writes: None,
        address_table_accounts: vec![],
        program_ids: vec![],
        program_data_accounts: vec![],
        semantic_dependency_accounts: vec![table.clone()],
        excluded_dependency_accounts: vec![],
        fee_only_payer: false,
        succeeded: true,
        is_vote: false,
    };
    let mut bundle = original.clone();
    bundle.writable_accounts.clear();
    bundle.address_table_accounts = vec![table];
    for (mode, refused) in [(WhatIfMode::After, true), (WhatIfMode::Replace, false)] {
        let input = request(WhatIf {
            mode,
            transactions_base64: Some(vec!["AQ==".into()]),
            state_overrides: vec![],
        });
        let result = dependency_target(&input, &original, Some(&bundle));
        assert_eq!(result.is_err(), refused, "{mode:?}");
    }
}
