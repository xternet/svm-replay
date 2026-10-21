use super::*;

#[test]
fn the_boundary_block_holds_only_what_happened_before_the_position() {
    let mut history = History::new();
    let block = read(&mut history, json!({"kind":"boundaryBlock"})).unwrap();
    assert_eq!(
        (block["slot"].clone(), block["parentSlot"].clone()),
        (json!(100), json!(99))
    );
    assert_eq!(
        (block["blockHeight"].clone(), block["blockTime"].clone()),
        (json!(90), json!(1000))
    );
    assert_eq!(
        block["previousBlockhash"],
        history.block["previousBlockhash"]
    );
    assert!(
        block.get("blockhash").is_none(),
        "the canonical hash is never the fork's"
    );
    let transactions = block["transactions"].as_array().unwrap();
    assert_eq!(transactions.len(), 2);
    for (index, (record, archived)) in transactions
        .iter()
        .zip(history.block["transactions"].as_array().unwrap())
        .enumerate()
    {
        assert_eq!(
            record["transactionBase64"],
            json!(svm_replay_protocol::transaction::encode(archived).unwrap())
        );
        assert_eq!(
            (record["slot"].clone(), record["meta"].clone()),
            (json!(100), archived["meta"].clone())
        );
        assert_eq!(record["transactionIndex"], json!(index));
    }
    assert!(
        history.reads.is_empty(),
        "the boundary block needs no archive read"
    );
}

#[test]
fn transactions_at_or_after_the_position_did_not_happen_and_earlier_ones_come_from_the_archive() {
    let mut history = History::new();
    let found = read(
        &mut history,
        json!({"kind":"transaction","signature":sig(2)}),
    )
    .unwrap();
    assert_eq!(found["slot"], json!(100));
    assert_eq!(found["transactionIndex"], json!(1));
    assert!(read(
        &mut history,
        json!({"kind":"transaction","signature":sig(3)})
    )
    .unwrap()
    .is_null());
    assert!(history.reads.is_empty());
    let error = read(
        &mut history,
        json!({"kind":"transaction","signature":sig(9)}),
    )
    .unwrap_err();
    assert_eq!(
        error.code, "SOURCE_TRANSPORT",
        "an offline archive is an error, never an absence"
    );
    let boundary_only = json!({"kind":"transaction","signature":sig(9),"history":false});
    let reads = history.reads.len();
    assert!(read(&mut history, boundary_only).unwrap().is_null());
    assert_eq!(
        history.reads.len(),
        reads,
        "a lookup without history reads no archive"
    );

    let mut older = entry(9, &[key(9), key(2)], b"o");
    older["slot"] = json!(42);
    older["blockTime"] = json!(500);
    let mut history = History::new().answer("transaction", older.clone());
    let found = read(
        &mut history,
        json!({"kind":"transaction","signature":sig(9)}),
    )
    .unwrap();
    assert_eq!(
        (found["slot"].clone(), found["blockTime"].clone()),
        (json!(42), json!(500))
    );
    assert!(
        found["transactionIndex"].is_null(),
        "no index the provider did not report"
    );
    older["transactionIndex"] = json!(7);
    let mut history = History::new().answer("transaction", older.clone());
    let query = json!({"kind":"transaction","signature":sig(9)});
    assert_eq!(
        read(&mut history, query).unwrap()["transactionIndex"],
        json!(7)
    );
    older["transactionIndex"] = json!("7");
    let mut history = History::new().answer("transaction", older.clone());
    let query = json!({"kind":"transaction","signature":sig(9)});
    assert_eq!(
        read(&mut history, query).unwrap_err().code,
        "SOURCE_INTEGRITY"
    );
    older.as_object_mut().unwrap().remove("transactionIndex");
    older["slot"] = json!(101);
    let mut history = History::new().answer("transaction", older);
    assert!(read(
        &mut history,
        json!({"kind":"transaction","signature":sig(9)})
    )
    .unwrap()
    .is_null());
    let mut history = History::new().answer("transaction", Value::Null);
    assert!(read(
        &mut history,
        json!({"kind":"transaction","signature":sig(9)})
    )
    .unwrap()
    .is_null());
}

#[test]
fn signatures_list_the_boundary_block_before_the_position_then_page_earlier_history() {
    let page = json!([{"signature":sig(9),"slot":42,"err":null,"memo":null,"blockTime":500,"confirmationStatus":"finalized"}]);
    let query = |before: Option<String>, until: Option<String>, limit: u64| json!({"kind":"signatures","address":address(),"before":before,"until":until,"limit":limit});
    let mut history = History::new().answer("signatures", page.clone());
    let listed = read(&mut history, query(None, None, 10)).unwrap();
    assert_eq!(listed[0]["signature"], json!(sig(1)));
    assert_eq!(listed[0]["memo"], json!("[2] hi"));
    assert_eq!(listed[1], page[0]);
    assert_eq!(
        listed.as_array().unwrap().len(),
        2,
        "s3 and s4 did not happen on this fork"
    );
    assert_eq!(
        history.reads,
        vec![
            json!({"kind":"signatures","address":address(),"before":sig(1),"until":null,"limit":9})
        ]
    );

    let mut history = History::new();
    assert_eq!(
        read(&mut history, query(None, None, 1))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        read(&mut history, query(Some(sig(3)), None, 10)).unwrap(),
        json!([])
    );
    assert_eq!(
        read(&mut history, query(None, Some(sig(1)), 10)).unwrap(),
        json!([])
    );
    assert!(
        history.reads.is_empty(),
        "satisfied from the boundary block alone"
    );

    let mut history = History::new().answer("signatures", json!([{"signature":sig(9),"slot":100}]));
    let error = read(&mut history, query(None, None, 10)).unwrap_err();
    assert_eq!(
        error.code, "SOURCE_INTEGRITY",
        "an archive page reaching the boundary is refused"
    );
}

#[test]
fn block_lists_and_blocks_stop_before_the_boundary_slot() {
    let mut history = History::new().answer("blocks", json!([95, 97, 99]));
    assert_eq!(
        read(&mut history, json!({"kind":"blocks","start":90,"end":150})).unwrap(),
        json!([95, 97, 99])
    );
    assert_eq!(
        history.reads,
        vec![json!({"kind":"blocks","start":90,"end":99})]
    );
    assert_eq!(
        read(&mut history, json!({"kind":"blocks","start":100,"end":150})).unwrap(),
        json!([])
    );
    let mut history = History::new().answer("blocks", json!([101]));
    assert!(read(&mut history, json!({"kind":"blocks","start":90,"end":150})).is_err());
    let error = read(&mut history, json!({"kind":"block","slot":100})).unwrap_err();
    assert_eq!(error.code, "INVALID_QUERY");
    assert_eq!(
        read(&mut history, json!({"kind":"block","slot":99}))
            .unwrap_err()
            .code,
        "SOURCE_TRANSPORT"
    );
}
