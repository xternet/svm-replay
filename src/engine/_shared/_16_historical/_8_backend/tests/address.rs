use super::*;

fn listed(seed: u8, slot: u64, index: u64) -> Value {
    json!({"signature":sig(seed),"slot":slot,"transactionIndex":index,"err":null,"memo":null,
        "blockTime":1000,"confirmationStatus":"finalized"})
}

fn query(order: &str) -> Value {
    json!({"kind":"transactionsForAddress","address":address(),"sortOrder":order,
        "tokenAccounts":"none","status":"any","limit":10})
}

#[test]
fn oldest_first_history_ends_at_the_fork_position() {
    let mut history = History::new().answer(
        "transactionsForAddress",
        json!({"data":[listed(9, 97, 0),
        listed(1, 100, 0), listed(2, 100, 1), listed(3, 100, 2)],"paginationToken":"100:2"}),
    );
    let page = read(&mut history, query("asc")).unwrap();
    assert_eq!(
        page,
        json!({"data":[listed(9, 97, 0), listed(1, 100, 0), listed(2, 100, 1)],
        "paginationToken":null})
    );
    assert_eq!(
        history.reads,
        vec![json!({"kind":"transactionsForAddress","address":address(),
        "sortOrder":"asc","tokenAccounts":"none","status":"any","limit":10,"paginationToken":null,
        "slot":null,"blockTime":null,"untilSlot":100})]
    );
}

#[test]
fn slot_time_and_status_filters_reach_the_provider_and_bind_its_entries() {
    let mut filtered = query("asc");
    filtered["slot"] = json!({"gte":90,"lt":99});
    filtered["blockTime"] = json!({"eq":1000});
    filtered["status"] = json!("succeeded");
    let mut history = History::new().answer(
        "transactionsForAddress",
        json!({"data":[listed(9, 97, 0)],"paginationToken":null}),
    );
    let page = read(&mut history, filtered.clone()).unwrap();
    assert_eq!(page["data"], json!([listed(9, 97, 0)]));
    assert_eq!(
        (
            history.reads[0]["slot"].clone(),
            history.reads[0]["blockTime"].clone(),
            history.reads[0]["status"].clone()
        ),
        (
            json!({"gte":90,"lt":99}),
            json!({"eq":1000}),
            json!("succeeded")
        )
    );
    let mut failed = listed(8, 95, 0);
    failed["err"] = json!({"InstructionError":[0,"InvalidArgument"]});
    let mut late = listed(8, 95, 0);
    late["blockTime"] = json!(1001);
    for entry in [listed(8, 89, 0), listed(8, 99, 0), failed, late] {
        let mut history = History::new().answer(
            "transactionsForAddress",
            json!({"data":[entry],"paginationToken":null}),
        );
        assert_eq!(
            read(&mut history, filtered.clone()).unwrap_err().code,
            "SOURCE_INTEGRITY"
        );
    }
    for (field, value) in [
        ("slot", json!({"gte":"90"})),
        ("slot", json!({"between":1})),
        ("blockTime", json!([1])),
    ] {
        let mut malformed = query("asc");
        malformed[field] = value;
        let mut history = History::new();
        assert_eq!(
            read(&mut history, malformed).unwrap_err().code,
            "INVALID_QUERY"
        );
        assert!(history.reads.is_empty());
    }
}

#[test]
fn newest_first_history_skips_what_did_not_happen_on_the_fork() {
    let mut history = History::new().answer(
        "transactionsForAddress",
        json!({"data":[listed(4, 100, 3),
        listed(3, 100, 2), listed(2, 100, 1), listed(9, 97, 0)],"paginationToken":"97:0"}),
    );
    let mut wanted = query("desc");
    wanted["paginationToken"] = json!("100:4");
    wanted["tokenAccounts"] = json!("all");
    let page = read(&mut history, wanted).unwrap();
    assert_eq!(
        page,
        json!({"data":[listed(2, 100, 1), listed(9, 97, 0)],"paginationToken":"97:0"})
    );
    assert_eq!(
        (
            history.reads[0]["paginationToken"].clone(),
            history.reads[0]["tokenAccounts"].clone()
        ),
        (json!("100:4"), json!("all"))
    );
}

#[test]
fn provider_history_that_contradicts_the_boundary_block_or_its_order_is_refused() {
    for (order, data, token) in [
        ("asc", vec![listed(7, 100, 1)], Value::Null),
        ("asc", vec![listed(9, 101, 0)], Value::Null),
        (
            "asc",
            vec![listed(1, 100, 0), listed(9, 97, 0)],
            Value::Null,
        ),
        (
            "desc",
            vec![listed(9, 97, 0), listed(2, 100, 1)],
            Value::Null,
        ),
        ("asc", vec![listed(9, 97, 0)], json!("97")),
    ] {
        let mut history = History::new().answer(
            "transactionsForAddress",
            json!({"data":data,"paginationToken":token}),
        );
        assert_eq!(
            read(&mut history, query(order)).unwrap_err().code,
            "SOURCE_INTEGRITY"
        );
    }
    let mut history = History::new();
    let mut unbounded = query("asc");
    unbounded["limit"] = json!(1001);
    assert_eq!(
        read(&mut history, unbounded).unwrap_err().code,
        "INVALID_QUERY"
    );
    assert!(history.reads.is_empty());
}

/// Review M62-A3: a provider page larger than its requested limit is out of contract.
#[test]
fn a_page_larger_than_its_limit_is_refused() {
    let mut wanted = query("asc");
    wanted["limit"] = json!(1);
    let mut history = History::new().answer(
        "transactionsForAddress",
        json!({"data":[listed(8, 96, 0), listed(9, 97, 0)],"paginationToken":null}),
    );
    assert_eq!(
        read(&mut history, wanted).unwrap_err().code,
        "SOURCE_INTEGRITY"
    );
}

/// Review M62-A4: rows and the next token lie strictly past the request's exclusive cursor in
/// the requested order, and the next token is not behind the page's own last row.
#[test]
fn a_page_that_does_not_advance_past_its_cursor_is_refused() {
    for (order, cursor, data, next) in [
        ("asc", json!("97:0"), vec![listed(9, 97, 0)], json!("97:0")),
        ("asc", json!("97:0"), vec![listed(9, 98, 0)], json!("96:5")),
        ("asc", Value::Null, vec![listed(9, 98, 0)], json!("97:0")),
        ("desc", json!("98:0"), vec![listed(9, 98, 0)], json!("97:0")),
        ("desc", json!("98:0"), vec![listed(9, 97, 0)], json!("98:0")),
    ] {
        let mut wanted = query(order);
        wanted["paginationToken"] = cursor;
        let mut history = History::new().answer(
            "transactionsForAddress",
            json!({"data":data,"paginationToken":next}),
        );
        let error = read(&mut history, wanted).unwrap_err();
        assert_eq!(error.code, "SOURCE_INTEGRITY", "{order} {next}");
    }
    let mut wanted = query("asc");
    wanted["paginationToken"] = json!("96:0");
    let mut history = History::new().answer(
        "transactionsForAddress",
        json!({"data":[listed(9, 97, 0)],"paginationToken":"97:0"}),
    );
    let page = read(&mut history, wanted).unwrap();
    assert_eq!(page["paginationToken"], json!("97:0"));
}
