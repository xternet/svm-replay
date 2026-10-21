use super::tokens::{b58, balance, boundary, holders, listed, page, TOKEN};
use super::*;

/// Review M62-A1: an owner page answers for the slot it was asked about, or proves nothing.
#[test]
fn an_owner_page_for_another_or_no_slot_is_refused() {
    let (mint, owner) = (b58(30), b58(31));
    for context in [json!({"slot":999}), json!({}), Value::Null] {
        let mut answer = page(vec![listed(10, &owner, &mint)], Value::Null);
        answer["context"] = context;
        let mut history = boundary().then("tokenAccountsByOwner", answer);
        let query = json!({"kind":"tokenAccountsByOwner","owner":owner,"mint":mint});
        let error = read(&mut history, query).unwrap_err();
        assert_eq!(error.code, "SOURCE_CONTEXT_MISMATCH");
    }
}

/// Review M62-A1: neither a mint's presence nor its absence counts from another slot.
#[test]
fn a_mint_image_for_another_or_no_slot_proves_nothing() {
    let image = json!({"lamports":1461600,"owner":TOKEN,"executable":false,"rentEpoch":0,
        "space":82,"data":["","base64"]});
    for value in [Value::Null, image] {
        for context in [json!({"slot":999}), json!({})] {
            let mut history = boundary()
                .answer("accountAtSlot", json!({"context":context,"value":value}))
                .answer("tokenHolders", holders(&[(20, "5")]));
            let query = json!({"kind":"tokenHolders","mint":b58(30),"limit":3});
            let error = read(&mut history, query).unwrap_err();
            assert_eq!(error.code, "SOURCE_CONTEXT_MISMATCH");
        }
    }
}

/// Review M62-A2: a prefix balance without its optional owner may be the owner's account,
/// so it stays in the candidate superset the embedder narrows with current images.
#[test]
fn a_prefix_balance_without_its_owner_stays_a_candidate() {
    let (mint, owner) = (b58(30), b58(31));
    let mut unowned = balance(1, &mint, &owner, "7");
    unowned.as_object_mut().unwrap().remove("owner");
    let mut history = boundary();
    history.block["transactions"][1]["meta"]["postTokenBalances"] = json!([unowned]);
    let mut history = history.then("tokenAccountsByOwner", page(vec![], Value::Null));
    let query = json!({"kind":"tokenAccountsByOwner","owner":owner,"mint":mint});
    let found = read(&mut history, query).unwrap();
    let accounts = found["accounts"].as_array().unwrap();
    assert!(accounts.contains(&json!(b58(6))), "{found}");
}
