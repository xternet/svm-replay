use super::_1_payload::{Bundle, Overrides};
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use svm_replay_protocol::WhatIfMode;

fn wire(seed: u8) -> String {
    let mut bytes = vec![1];
    bytes.extend_from_slice(&[seed; 64]);
    bytes.extend_from_slice(&[1, 0, 1, 2]);
    bytes.extend_from_slice(&[7; 32]);
    bytes.extend_from_slice(&[8; 32]);
    bytes.extend_from_slice(&[9; 32]);
    bytes.extend_from_slice(&[1, 1, 1, 0, 0]);
    STANDARD.encode(bytes)
}

const ACCOUNT: &str = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";

#[test]
fn bundles_are_one_object_or_an_array() {
    let one = bundle(&format!(r#"{{"transactionBase64":"{}"}}"#, wire(1))).unwrap();
    assert_eq!((one.transactions.len(), one.array), (1, false));
    let two = bundle(&format!(
        r#"[{{"transactionBase64":"{}"}},{{"transactionBase64":"{}"}}]"#,
        wire(1),
        wire(2)
    ))
    .unwrap();
    assert_eq!(
        (two.transactions, two.array),
        (vec![wire(1), wire(2)], true)
    );
    let many = format!(
        "[{}]",
        vec![format!(r#"{{"transactionBase64":"{}"}}"#, wire(3)); 17].join(",")
    );
    for bad in [
        "[]",
        &many,
        r#"[{"transactionBase64":"AQ=="}]"#,
        r#"{"tx":"x"}"#,
    ] {
        assert!(bundle(bad).is_err(), "{bad}");
    }
}

#[test]
fn overrides_keep_the_old_shape_and_add_eth_call_fields() {
    let legacy = format!(r#"[{{"pubkey":"{ACCOUNT}","lamports":"10"}}]"#);
    assert!(matches!(overrides(&legacy).unwrap(), Overrides::Legacy(_)));
    let state = format!(r#"[{{"pubkey":"{ACCOUNT}","owner":"{ACCOUNT}","lamports":"10"}}]"#);
    assert!(matches!(overrides(&state).unwrap(), Overrides::State(ref v) if v.len() == 1));
    let directory = tempfile::tempdir().unwrap();
    let elf = directory.path().join("arb.so");
    std::fs::write(&elf, b"\x7fELF-program-bytes").unwrap();
    let text = directory.path().join("notes.txt");
    std::fs::write(&text, b"hello").unwrap();
    let program = serde_json::json!([{"pubkey": ACCOUNT,
        "program": format!("@{}", elf.display())}])
    .to_string();
    let Overrides::State(entries) = overrides(&program).unwrap() else {
        panic!("program is a state override")
    };
    assert_eq!(
        entries[0].owner.as_deref(),
        Some("BPFLoader2111111111111111111111111111111111")
    );
    assert_eq!(entries[0].executable, Some(true));
    assert_eq!(
        entries[0].data_base64.as_deref(),
        Some(STANDARD.encode(b"\x7fELF-program-bytes").as_str())
    );
    for bad in [
        serde_json::json!([{"pubkey": ACCOUNT,
            "program": format!("@{}", text.display())}])
        .to_string(),
        serde_json::json!([{"pubkey": ACCOUNT,
            "program": elf.display().to_string()}])
        .to_string(),
        serde_json::json!([{"pubkey": ACCOUNT,
            "program": format!("@{}", elf.display()), "owner": ACCOUNT}])
        .to_string(),
        format!(r#"[{{"pubkey":"{ACCOUNT}","owner":"{ACCOUNT}","color":"red"}}]"#),
        format!(r#"[{{"pubkey":"{ACCOUNT}"}}]"#),
    ] {
        assert!(overrides(&bad).is_err(), "{bad}");
    }
}

#[test]
fn old_shaped_requests_stay_on_the_old_path() {
    let one = Bundle {
        transactions: vec![wire(1)],
        array: false,
    };
    let legacy = Overrides::Legacy(serde_json::json!([{"pubkey":ACCOUNT,"lamports":"1"}]));
    let (replacement, old, what_if) =
        variants(Some(one.clone()), None, Some(legacy.clone())).unwrap();
    assert_eq!(replacement, Some(wire(1)));
    assert!(old.is_some() && what_if.is_none());
    let after = variants(None, Some(one.clone()), Some(legacy))
        .unwrap()
        .2
        .unwrap();
    assert_eq!(after.mode, WhatIfMode::After);
    assert_eq!(after.state_overrides[0].lamports.as_deref(), Some("1"));
    let array = Bundle {
        transactions: vec![wire(1), wire(2)],
        array: true,
    };
    let replace = variants(Some(array), None, None).unwrap().2.unwrap();
    assert_eq!(
        (replace.mode, replace.transactions_base64.unwrap().len()),
        (WhatIfMode::Replace, 2)
    );
    let state = overrides(&format!(
        r#"[{{"pubkey":"{ACCOUNT}","owner":"{ACCOUNT}"}}]"#
    ))
    .unwrap();
    let rerun = variants(None, None, Some(state.clone()))
        .unwrap()
        .2
        .unwrap();
    assert_eq!(
        (rerun.mode, rerun.transactions_base64),
        (WhatIfMode::Replace, None)
    );
    let instead = variants(Some(one.clone()), None, Some(state))
        .unwrap()
        .2
        .unwrap();
    assert_eq!(instead.transactions_base64, Some(vec![wire(1)]));
    assert!(variants(Some(one.clone()), Some(one), None).is_err());
}
