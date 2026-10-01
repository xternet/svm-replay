use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use svm_replay_engine::shared::history::{
    account_data, classify_account, program_data_address, Roles,
};

fn account() -> serde_json::Value {
    json!({"pubkey":"11111111111111111111111111111111","sourceSlot":100,"role":"application","presence":"present",
      "owner":"11111111111111111111111111111111","lamports":"18446744073709551615","rentEpoch":"18446744073709551615",
      "executable":false,"dataBase64":""})
}
#[test]
fn unchanged_old_account_uses_queried_asof_image_and_exact_u64() {
    let a = classify_account(
        account(),
        "11111111111111111111111111111111",
        100,
        &Roles::default(),
    )
    .unwrap();
    assert_eq!(a["lamports"], "18446744073709551615");
    assert!(account_data(&a).unwrap().is_empty());
    assert!(classify_account(
        account(),
        "11111111111111111111111111111111",
        101,
        &Roles::default()
    )
    .is_err());
}
#[test]
fn absent_application_is_not_missing_source_or_absent_program() {
    let key = "11111111111111111111111111111111";
    let a = json!({"pubkey":key,"sourceSlot":100,"role":"application","presence":"absent"});
    assert!(classify_account(a.clone(), key, 100, &Roles::default()).is_ok());
    let mut roles = Roles::default();
    roles.programs.insert(key.into());
    assert_eq!(
        classify_account(a, key, 100, &roles).unwrap_err().code,
        "UNSUPPORTED_REQUIRED_ACCOUNT"
    );
}

#[test]
fn absent_required_sysvar_has_structured_boundary() {
    let key = "SysvarS1otHashes111111111111111111111111111";
    let mut roles = Roles::default();
    roles.sysvars.insert(key.into());
    let value = json!({"pubkey":key,"sourceSlot":100,"role":"sysvar","presence":"absent"});
    let error = classify_account(value, key, 100, &roles).unwrap_err();
    assert_eq!(error.code, "UNSUPPORTED_REQUIRED_ACCOUNT");
    assert_eq!(
        error.details,
        Some(json!({"pubkey":key,"slot":100,"role":"sysvar"}))
    );
}
#[test]
fn programdata_link_comes_from_executable_loader_bytes_not_guessed_addresses() {
    let mut a = account();
    a["owner"] = json!("BPFLoaderUpgradeab1e11111111111111111111111");
    a["executable"] = json!(true);
    let mut data = 2u32.to_le_bytes().to_vec();
    data.extend([7u8; 32]);
    a["dataBase64"] = json!(STANDARD.encode(&data));
    assert_eq!(
        program_data_address(&a).unwrap(),
        Some(bs58::encode([7u8; 32]).into_string())
    );
    a["dataBase64"] = json!(STANDARD.encode(&data[..35]));
    assert!(program_data_address(&a).is_err());
    a["owner"] = json!("11111111111111111111111111111111");
    assert_eq!(program_data_address(&a).unwrap(), None);
}
#[test]
fn reject_noncanonical_account_values() {
    for (field, value) in [
        ("lamports", json!("18446744073709551616")),
        ("rentEpoch", json!("01")),
        ("dataBase64", json!("AB==")),
        ("owner", json!("x")),
        ("executable", json!(1)),
    ] {
        let mut a = account();
        a[field] = value;
        assert!(
            classify_account(
                a,
                "11111111111111111111111111111111",
                100,
                &Roles::default()
            )
            .is_err(),
            "{field}"
        );
    }
}

#[path = "history_cases/acquisition_budget.rs"]
mod acquisition_budget;
