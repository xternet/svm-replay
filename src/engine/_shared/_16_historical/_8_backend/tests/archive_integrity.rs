use super::*;

fn finalized(seed: u8, slot: u64) -> Value {
    json!({"signature":sig(seed),"slot":slot,"err":null,"memo":null,"blockTime":500,"confirmationStatus":"finalized"})
}

fn listing() -> Value {
    json!({"kind":"signatures","address":address(),"before":null,"until":null,"limit":10})
}

#[test]
fn malformed_archive_answers_are_refused_never_repaired() {
    let mut confirmed = finalized(9, 42);
    confirmed["confirmationStatus"] = json!("confirmed");
    let mut no_status = finalized(9, 42);
    no_status.as_object_mut().unwrap().remove("err");
    let mut bad_name = finalized(9, 42);
    bad_name["signature"] = json!("bad");
    for page in [
        json!([finalized(9, 42), finalized(9, 41)]),
        json!([finalized(9, 41), finalized(8, 42)]),
        json!([bad_name]),
        json!([no_status]),
        json!([confirmed]),
        json!([finalized(1, 42)]),
    ] {
        let mut history = History::new().answer("signatures", page.clone());
        assert_eq!(
            read(&mut history, listing()).unwrap_err().code,
            "SOURCE_INTEGRITY",
            "{page}"
        );
    }
    for slots in [json!([97, 95]), json!([95, 95]), json!(["95"])] {
        let mut history = History::new().answer("blocks", slots.clone());
        let query = json!({"kind":"blocks","start":90,"end":99});
        assert_eq!(
            read(&mut history, query).unwrap_err().code,
            "SOURCE_INTEGRITY",
            "{slots}"
        );
    }
    let mut history = History::new();
    history.block["transactions"][0]["transaction"]["message"]["accountKeys"] = Value::Null;
    assert_eq!(
        read(&mut history, listing()).unwrap_err().code,
        "SOURCE_INTEGRITY"
    );
    let mut history = History::new();
    history.block["transactions"][1]["transaction"]["signatures"][0] = json!("bad");
    let query = json!({"kind":"transaction","signature":sig(9)});
    assert_eq!(
        read(&mut history, query).unwrap_err().code,
        "SOURCE_INTEGRITY"
    );
    assert!(
        history.reads.is_empty(),
        "nothing is read past malformed retained input"
    );
}

/// An empty boundary block anchors at the nearest earlier block with a transaction: all of
/// it precedes the position, and older pages continue before its first signature.
#[test]
fn an_empty_boundary_block_lists_from_the_nearest_earlier_block_with_a_transaction() {
    let anchor = json!({"blockTime":970,"transactions":[entry(5, &[key(5), key(2)], b"a"),
        entry(6, &[key(6), key(7)], b"b"), entry(7, &[key(8), key(2)], b"c")]});
    let mut history = History::new()
        .answer("blocks", json!([97, 99]))
        .answer("signatures", json!([finalized(9, 42)]))
        .archived(99, json!({"blockTime":990,"transactions":[]}))
        .archived(97, anchor);
    history.block["transactions"] = json!([]);
    history.position = 0;
    let listed = read(&mut history, listing()).unwrap();
    let names: Vec<_> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["signature"].clone())
        .collect();
    assert_eq!(names, vec![json!(sig(7)), json!(sig(5)), json!(sig(9))]);
    assert_eq!(
        (listed[0]["slot"].clone(), listed[0]["blockTime"].clone()),
        (json!(97), json!(970))
    );
    assert_eq!(
        history.reads,
        vec![
            json!({"kind":"blocks","start":0,"end":99}),
            json!({"block":99}),
            json!({"block":97}),
            json!({"kind":"signatures","address":address(),"before":sig(5),"until":null,"limit":8})
        ]
    );

    // No block with a transaction within the bounds is the adapter's limit, named as such.
    let mut history = History::new().answer("blocks", json!([]));
    history.block["transactions"] = json!([]);
    history.position = 0;
    assert_eq!(
        read(&mut history, listing()).unwrap_err().code,
        "ARCHIVE_CURSOR_UNAVAILABLE"
    );
    // An unreadable earlier block stays the source's own failure.
    let mut history = History::new().answer("blocks", json!([97]));
    history.block["transactions"] = json!([]);
    history.position = 0;
    assert_eq!(
        read(&mut history, listing()).unwrap_err().code,
        "SOURCE_TRANSPORT"
    );
}
