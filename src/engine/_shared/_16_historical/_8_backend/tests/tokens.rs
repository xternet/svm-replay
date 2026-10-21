use super::*;

pub(super) const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

pub(super) fn b58(seed: u8) -> String {
    bs58::encode(key(seed)).into_string()
}

pub(super) fn balance(index: u64, mint: &str, owner: &str, amount: &str) -> Value {
    json!({"accountIndex":index,"mint":mint,"owner":owner,"programId":TOKEN,
        "uiTokenAmount":{"amount":amount,"decimals":0,"uiAmount":null,"uiAmountString":amount}})
}

/// The boundary block's token balances: in the prefix, s1 moves key(2) from the owner to
/// another wallet and s2 creates key(6) for the owner; in the suffix, s3 creates key(3) for
/// the owner, which never happened on the fork.
pub(super) fn boundary() -> History {
    let (mint, owner, other) = (b58(30), b58(31), b58(32));
    let mut history = History::new();
    let transactions = history.block["transactions"].as_array_mut().unwrap();
    transactions[0]["meta"]["preTokenBalances"] = json!([balance(1, &mint, &owner, "3")]);
    transactions[0]["meta"]["postTokenBalances"] = json!([balance(1, &mint, &other, "3")]);
    transactions[1]["meta"]["preTokenBalances"] = json!([]);
    transactions[1]["meta"]["postTokenBalances"] = json!([balance(1, &mint, &owner, "7")]);
    transactions[2]["meta"]["postTokenBalances"] = json!([balance(0, &mint, &owner, "9")]);
    history
}

pub(super) fn listed(pubkey: u8, owner: &str, mint: &str) -> Value {
    json!({"pubkey":b58(pubkey),"account":{"lamports":2039280,"owner":TOKEN,"executable":false,
        "rentEpoch":0,"space":165,"data":{"program":"spl-token","space":165,"parsed":{"type":"account",
        "info":{"isNative":false,"mint":mint,"owner":owner,"state":"initialized",
        "tokenAmount":{"amount":"1","decimals":0,"uiAmount":1.0,"uiAmountString":"1"}}}}}})
}

pub(super) fn page(items: Vec<Value>, next: Value) -> Value {
    json!({"context":{"slot":99,"apiVersion":"2.0"},"value":items,"pageKey":next})
}

#[test]
fn owner_candidates_are_every_listed_page_at_the_parent_slot_and_the_prefix_changes() {
    let (mint, owner) = (b58(30), b58(31));
    let mut history = boundary()
        .then(
            "tokenAccountsByOwner",
            page(vec![listed(10, &owner, &mint)], json!("next-1")),
        )
        .then(
            "tokenAccountsByOwner",
            page(vec![listed(11, &owner, &b58(33))], Value::Null),
        );
    let found = read(
        &mut history,
        json!({"kind":"tokenAccountsByOwner","owner":owner,"programId":TOKEN}),
    )
    .unwrap();
    let mut expected = vec![b58(10), b58(11), b58(2), b58(6)];
    expected.sort();
    assert_eq!(found, json!({"accounts":expected,"slot":99,"pages":2}));
    let query = |page_key: Value| {
        json!({"kind":"tokenAccountsByOwner","owner":owner,"mint":null,
        "programId":TOKEN,"slot":99,"pageKey":page_key})
    };
    assert_eq!(
        history.reads,
        vec![query(Value::Null), query(json!("next-1"))]
    );
}

#[test]
fn a_mint_filter_reaches_the_provider_and_narrows_the_prefix() {
    let (mint, owner) = (b58(30), b58(31));
    let mut history = boundary().then("tokenAccountsByOwner", page(vec![], Value::Null));
    let found = read(
        &mut history,
        json!({"kind":"tokenAccountsByOwner","owner":owner,"mint":b58(33)}),
    )
    .unwrap();
    assert_eq!(found["accounts"], json!([]));
    assert_eq!(history.reads[0]["mint"], json!(b58(33)));
    let mut history = boundary().then("tokenAccountsByOwner", page(vec![], Value::Null));
    let found = read(
        &mut history,
        json!({"kind":"tokenAccountsByOwner","owner":owner,"mint":mint}),
    )
    .unwrap();
    assert_eq!(found["accounts"].as_array().unwrap().len(), 2);
}

#[test]
fn a_provider_list_that_contradicts_its_query_is_refused() {
    let (mint, owner) = (b58(30), b58(31));
    let other_owner = page(vec![listed(10, &b58(34), &mint)], Value::Null);
    let other_mint = page(vec![listed(10, &owner, &b58(33))], Value::Null);
    let repeated = page(vec![], json!("same"));
    let no_list = json!({"context":{"slot":99},"value":null,"pageKey":null});
    for (query, answers) in [
        (
            json!({"kind":"tokenAccountsByOwner","owner":owner,"programId":TOKEN}),
            vec![other_owner],
        ),
        (
            json!({"kind":"tokenAccountsByOwner","owner":owner,"mint":mint}),
            vec![other_mint],
        ),
        (
            json!({"kind":"tokenAccountsByOwner","owner":owner,"programId":TOKEN}),
            vec![repeated.clone(), repeated],
        ),
        (
            json!({"kind":"tokenAccountsByOwner","owner":owner,"programId":TOKEN}),
            vec![no_list],
        ),
    ] {
        let mut history = answers.into_iter().fold(boundary(), |history, answer| {
            history.then("tokenAccountsByOwner", answer)
        });
        assert_eq!(
            read(&mut history, query).unwrap_err().code,
            "SOURCE_INTEGRITY"
        );
    }
    let mut history = boundary();
    let both = json!({"kind":"tokenAccountsByOwner","owner":owner,"mint":mint,"programId":TOKEN});
    assert_eq!(read(&mut history, both).unwrap_err().code, "INVALID_QUERY");
    assert!(history.reads.is_empty());
}

pub(super) fn holders(entries: &[(u8, &str)]) -> Value {
    let list: Vec<Value> = entries
        .iter()
        .map(|(seed, amount)| {
            json!({"holder":b58(*seed),"owner":b58(40),
        "balanceRaw":amount,"balanceUi":amount})
        })
        .collect();
    json!({"mint":b58(30),"slot":99,"sortBy":"balance_desc","holders":list})
}

/// The mint's header at the parent slot: present, or absent when `value` is null.
fn mint_header(value: Value) -> Value {
    json!({"context":{"slot":99},"value":value})
}

fn present() -> Value {
    mint_header(
        json!({"lamports":1461600,"owner":TOKEN,"executable":false,"rentEpoch":0,
        "space":82,"data":["","base64"]}),
    )
}

#[test]
fn holders_rank_at_the_parent_slot_with_completeness_and_the_prefix_accounts() {
    let mut history = boundary()
        .answer("accountAtSlot", present())
        .answer("tokenHolders", holders(&[(20, "500"), (21, "300")]));
    let ranked = read(
        &mut history,
        json!({"kind":"tokenHolders","mint":b58(30),"limit":3}),
    )
    .unwrap();
    let mut touched = vec![b58(2), b58(6)];
    touched.sort();
    assert_eq!(
        ranked,
        json!({"slot":99,"mintAtParent":true,"complete":true,"floor":null,"touched":touched,
        "holders":[{"account":b58(20),"amount":"500"},{"account":b58(21),"amount":"300"}]})
    );
    assert_eq!(
        history.reads,
        vec![
            json!({"kind":"accountAtSlot","pubkey":b58(30),"slot":99}),
            json!({"kind":"tokenHolders","mint":b58(30),"slot":99,"limit":3})
        ]
    );
    // A full list may omit smaller holders: none can exceed its last balance.
    let ranked = read(
        &mut history,
        json!({"kind":"tokenHolders","mint":b58(30),"limit":2}),
    )
    .unwrap();
    assert_eq!(
        (ranked["complete"].clone(), ranked["floor"].clone()),
        (json!(false), json!("300"))
    );
}

#[test]
fn a_mint_absent_at_the_parent_slot_has_only_prefix_and_local_accounts() {
    let mut history = boundary().answer("accountAtSlot", mint_header(Value::Null));
    let ranked = read(
        &mut history,
        json!({"kind":"tokenHolders","mint":b58(30),"limit":3}),
    )
    .unwrap();
    let mut touched = vec![b58(2), b58(6)];
    touched.sort();
    assert_eq!(
        ranked,
        json!({"slot":99,"mintAtParent":false,"complete":true,"floor":null,"touched":touched,
        "holders":[]})
    );
    assert_eq!(
        history.reads.len(),
        1,
        "no ranking is read for an absent mint"
    );
}

#[test]
fn a_holder_ranking_off_its_documented_contract_is_refused() {
    let mut wrong_mint = holders(&[(20, "5")]);
    wrong_mint["mint"] = json!(b58(33));
    let mut wrong_slot = holders(&[(20, "5")]);
    wrong_slot["slot"] = json!(100);
    for answer in [
        holders(&[(20, "5"), (21, "6")]),
        holders(&[(20, "0")]),
        holders(&[(20, "x")]),
        holders(&[(20, "5"), (21, "4"), (22, "3"), (23, "2")]),
        wrong_mint,
        wrong_slot,
    ] {
        let mut history = boundary()
            .answer("accountAtSlot", present())
            .answer("tokenHolders", answer);
        let error = read(
            &mut history,
            json!({"kind":"tokenHolders","mint":b58(30),"limit":3}),
        )
        .unwrap_err();
        assert_eq!(error.code, "SOURCE_INTEGRITY");
    }
    let mut history = boundary().answer("accountAtSlot", mint_header(json!("present")));
    let error = read(
        &mut history,
        json!({"kind":"tokenHolders","mint":b58(30),"limit":3}),
    )
    .unwrap_err();
    assert_eq!(error.code, "SOURCE_INTEGRITY");
}
