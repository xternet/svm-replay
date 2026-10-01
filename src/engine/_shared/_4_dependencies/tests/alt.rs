use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use svm_replay_engine::shared::dependencies::alt::*;
use svm_replay_protocol::Digest;
fn table(id: &str, deactivation: u64) -> Value {
    let mut bytes = vec![0; 88];
    bytes[..4].copy_from_slice(&1u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&deactivation.to_le_bytes());
    json!({"pubkey":id,"sourceSlot":999,"role":"address-lookup-table","presence":"present","lamports":"1","rentEpoch":"0","owner":"AddressLookupTab1e1111111111111111111111111","executable":false,"dataBase64":STANDARD.encode(bytes)})
}
#[test]
fn active_preflight_hashes_sorted_unique_keys_and_preserves_u64_deactivation() {
    let proof =
        preflight_active_address_tables(&[table("b", u64::MAX), table("a", u64::MAX)]).unwrap();
    assert_eq!(
        proof,
        json!({"status":"NOT_REQUIRED","count":2,"pubkeysHash":Digest::of(b"a\nb\n"),"deactivationSlot":u64::MAX.to_string()})
    );
    assert_eq!(
        preflight_active_address_tables(&[table("b", 9007199254740993), table("a", 17)]).unwrap(),
        json!({"status":"REQUIRES_SLOT_HASHES","tables":[{"pubkey":"a","deactivationSlot":"17"},{"pubkey":"b","deactivationSlot":"9007199254740993"}]})
    );
    assert!(preflight_active_address_tables(&[]).is_err());
    assert!(
        preflight_active_address_tables(&[table("a", u64::MAX), table("a", u64::MAX)]).is_err()
    );
}

#[test]
fn deactivated_keys_use_the_prototypes_case_collation_not_hash_sort() {
    let input = vec![
        table("a2", 17),
        table("A2", 17),
        table("a1", 17),
        table("A1", 17),
    ];
    let result = preflight_active_address_tables(&input).unwrap();
    let keys: Vec<_> = result["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["pubkey"].as_str().unwrap())
        .collect();
    assert_eq!(keys, vec!["a1", "A1", "a2", "A2"]);
}
#[test]
fn recent_window_is_conservative_and_exact_slot_hashes_never_forges_age_proof() {
    for age in 1..=513 {
        let result = build_recent_address_table_proofs(&[table("a", 1000 - age)], 1000, 999, false);
        assert_eq!(result.is_ok(), age < 512, "age {age}");
    }
    assert!(build_recent_address_table_proofs(&[table("a", 1000)], 1000, 999, true).is_err());
    assert_eq!(
        build_recent_address_table_proofs(&[table("a", 100)], 1000, 999, true).unwrap(),
        Vec::<Value>::new()
    );
    assert_eq!(
        build_recent_address_table_proofs(&[table("a", u64::MAX)], 1000, 999, false).unwrap(),
        Vec::<Value>::new()
    );
    let original = table("a", 900);
    let proof = build_recent_address_table_proofs(&[original.clone()], 1000, 999, false).unwrap();
    let bytes = STANDARD
        .decode(original["dataBase64"].as_str().unwrap())
        .unwrap();
    assert_eq!(proof[0]["dataSha256"], json!(Digest::of(&bytes)));
}
#[test]
fn recent_parent_shape_limits_and_duplicate_guards_survive_exact_hashes() {
    for patch in [
        json!({"sourceSlot":998}),
        json!({"owner":"wrong"}),
        json!({"executable":true}),
        json!({"presence":"absent"}),
        json!({"dataBase64":"AAAA"}),
    ] {
        let mut value = table("a", 900);
        for (k, v) in patch.as_object().unwrap() {
            value[k] = v.clone();
        }
        for exact in [false, true] {
            let e =
                build_recent_address_table_proofs(&[value.clone()], 1000, 999, exact).unwrap_err();
            assert_eq!(e.code, "UNSUPPORTED_ALT_LIFECYCLE");
            assert_eq!(e.details.unwrap()["phase"], "PREFLIGHT");
        }
    }
    for size in [55, 57, 56 + 257 * 32] {
        let mut value = table("a", 900);
        let mut bytes = vec![0; size];
        bytes[..4].copy_from_slice(&1u32.to_le_bytes());
        value["dataBase64"] = json!(STANDARD.encode(bytes));
        assert!(build_recent_address_table_proofs(&[value], 1000, 999, true).is_err());
    }
    assert!(build_recent_address_table_proofs(
        &[table("a", 900), table("a", 900)],
        1000,
        999,
        true
    )
    .is_err());
    assert!(build_recent_address_table_proofs(&[], 999, 999, false).is_err());
}

#[test]
#[ignore = "explicit development differential requires Bun and SVM_REPLAY_PROTOTYPE"]
fn original_alt_proof_differential_includes_mixed_case_pubkey_order() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut cases = vec![];
    let mut actual = vec![];
    for slot in [u64::MAX, 1000, 999, 900, 489, 488, 0] {
        for exact in [false, true] {
            for mode in 0..5 {
                let mut accounts = vec![
                    table("a2", slot),
                    table("A2", slot),
                    table("a1", slot),
                    table("A1", slot),
                ];
                match mode {
                    0 => {}
                    1 => accounts[0]["sourceSlot"] = json!(998),
                    2 => accounts[0]["dataBase64"] = json!("AAAA"),
                    3 => accounts.push(accounts[0].clone()),
                    4 => accounts.clear(),
                    _ => unreachable!(),
                }
                actual.push(json!({"active":match preflight_active_address_tables(&accounts){Ok(v)=>v,Err(e)=>json!({"error":e.message})},"recent":match build_recent_address_table_proofs(&accounts,1000,999,exact){Ok(v)=>json!(v),Err(e)=>json!({"error":e.message})}}));
                cases.push(json!({"accounts":accounts,"exact":exact}));
            }
        }
    }
    let script = r#"const root=process.env.SVM_REPLAY_PROTOTYPE;const {preflightActiveAddressTables}=await import(root+'/poc/m2-realistic-defi/address-table-preflight.ts');const {buildRecentAddressTableProofs}=await import(root+'/poc/m6-prototype-breadth/recent-address-tables.ts');const apply=fn=>{try{return fn()}catch(e){return{error:e.reason===undefined?e.message:e.reason}}};const results=(await Bun.stdin.json()).map(c=>({active:apply(()=>preflightActiveAddressTables(c.accounts)),recent:apply(()=>buildRecentAddressTableProofs(c.accounts,1000,999,c.exact))}));console.log(JSON.stringify(results));"#;
    let mut child = Command::new("bun")
        .args(["--eval", script])
        .env(
            "SVM_REPLAY_PROTOTYPE",
            std::env::var("SVM_REPLAY_PROTOTYPE").expect("prototype path"),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
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
        assert_eq!(a, b, "case {i}");
    }
    assert_eq!(actual.len(), expected.as_array().unwrap().len());
}
