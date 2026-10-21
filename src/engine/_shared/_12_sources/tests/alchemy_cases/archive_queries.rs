use super::*;

const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// One archive read through a fresh source: its result and the exact request it sent.
fn read(query: Value, result: Value) -> (Value, Value) {
    let transport = Transport::values(vec![
        genesis(),
        json!({"jsonrpc":"2.0","id":1,"result":result}),
    ]);
    let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
    let value = source.archive(&query).unwrap();
    let request = transport.requests.lock().unwrap()[1].clone();
    (value, request)
}

fn owner(mint: Value, program: Value, page_key: Value) -> Value {
    json!({"kind":"tokenAccountsByOwner","owner":key(2),"mint":mint,"programId":program,"slot":99,"pageKey":page_key})
}

fn address(order: &str, token_accounts: &str, token: Value, limit: u64) -> Value {
    json!({"kind":"transactionsForAddress","address":key(2),"sortOrder":order,"tokenAccounts":token_accounts,
        "status":"any","limit":limit,"paginationToken":token,"slot":null,"blockTime":null,"untilSlot":100})
}

#[test]
fn history_filters_keep_the_boundary_bound() {
    let page = json!({"data":[],"paginationToken":null});
    let mut query = address("desc", "none", Value::Null, 5);
    query["slot"] = json!({"gte":90,"lte":200});
    query["blockTime"] = json!({"eq":5});
    let (_, request) = read(query, page.clone());
    assert_eq!(
        request["params"][1]["filters"],
        json!({"slot":{"gte":90,"lte":100},"blockTime":{"eq":5},"tokenAccounts":"none","status":"any"})
    );
    let mut query = address("desc", "none", Value::Null, 5);
    query["slot"] = json!({"lt":50});
    let (_, request) = read(query, page);
    assert_eq!(
        request["params"][1]["filters"]["slot"],
        json!({"lt":50,"lte":100})
    );
}

#[test]
fn owner_holder_and_address_queries_send_their_documented_requests() {
    let listed = json!({"context":{"slot":99},"value":[],"pageKey":null});
    let (value, request) = read(
        owner(Value::Null, json!(TOKEN), Value::Null),
        listed.clone(),
    );
    assert_eq!(value, listed);
    assert_eq!(
        request,
        json!({"jsonrpc":"2.0","id":1,"method":"getTokenAccountsByOwnerAtSlot",
        "params":[key(2),{"programId":TOKEN},{"slot":99,"pageLimit":10000,"encoding":"jsonParsed"}]})
    );
    let (_, request) = read(owner(json!(key(3)), Value::Null, json!("next")), listed);
    assert_eq!(
        request["params"],
        json!([key(2),{"mint":key(3)},
        {"slot":99,"pageLimit":10000,"encoding":"jsonParsed","pageKey":"next"}])
    );

    let ranked = json!({"mint":key(3),"slot":99,"sortBy":"balance_desc","holders":[]});
    let (value, request) = read(
        json!({"kind":"tokenHolders","mint":key(3),"slot":99,"limit":1000}),
        ranked.clone(),
    );
    assert_eq!(value, ranked);
    assert_eq!(
        request,
        json!({"jsonrpc":"2.0","id":1,"method":"getTokenHoldersAtSlot",
        "params":[key(3),99,1000,"balance_desc"]})
    );

    let absent = json!({"context":{"slot":99},"value":null});
    let (value, request) = read(
        json!({"kind":"accountAtSlot","pubkey":key(3),"slot":99}),
        absent.clone(),
    );
    assert_eq!(value, absent);
    assert_eq!(
        request,
        json!({"jsonrpc":"2.0","id":1,"method":"getAccountInfo",
        "params":[key(3),{"encoding":"base64","commitment":"finalized","slot":99}]})
    );

    let page = json!({"data":[],"paginationToken":null});
    let (value, request) = read(address("asc", "all", json!("99:3"), 10), page.clone());
    assert_eq!(value, page);
    assert_eq!(
        request,
        json!({"jsonrpc":"2.0","id":1,"method":"getTransactionsForAddress","params":[key(2),
        {"transactionDetails":"signatures","sortOrder":"asc","limit":10,"commitment":"finalized",
        "paginationToken":"99:3","filters":{"slot":{"lte":100},"tokenAccounts":"all","status":"any"}}]})
    );
}

#[test]
fn malformed_owner_holder_and_address_queries_never_reach_the_provider() {
    for query in [
        owner(json!(key(3)), json!(TOKEN), Value::Null),
        owner(Value::Null, Value::Null, Value::Null),
        owner(Value::Null, json!("not-a-key"), Value::Null),
        owner(Value::Null, json!(TOKEN), json!("")),
        json!({"kind":"tokenHolders","mint":key(3),"slot":99,"limit":10001}),
        json!({"kind":"tokenHolders","mint":key(3),"slot":99,"limit":0}),
        json!({"kind":"accountAtSlot","pubkey":"not-a-key","slot":99}),
        json!({"kind":"accountAtSlot","pubkey":key(3),"slot":-1}),
        address("sideways", "none", Value::Null, 10),
        address("asc", "some", Value::Null, 10),
        address("asc", "none", json!("99"), 10),
        address("asc", "none", Value::Null, 0),
        address("desc", "none", Value::Null, 1001),
        {
            let mut bad = address("desc", "none", Value::Null, 5);
            bad["slot"] = json!({"lte":-1});
            bad
        },
        {
            let mut bad = address("desc", "none", Value::Null, 5);
            bad["blockTime"] = json!({"near":5});
            bad
        },
    ] {
        let transport = Transport::values(vec![]);
        let source = AlchemySource::with_transport(config(), limits(), transport.clone()).unwrap();
        assert_eq!(
            source.archive(&query).unwrap_err().code,
            "INVALID_QUERY",
            "{query}"
        );
        assert!(transport.requests.lock().unwrap().is_empty());
    }
}
