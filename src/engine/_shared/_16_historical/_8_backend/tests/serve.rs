use super::*;

#[test]
fn requests_reject_unknown_operations_and_keep_the_id_for_the_reply() {
    let request = _0_request::decode(r#"{"id":7,"branch":2,"op":"fork"}"#)
        .ok()
        .unwrap();
    assert_eq!((request.id, request.branch), (7, 2));
    assert!(matches!(request.op, Op::Fork));
    let (id, error) = _0_request::decode(r#"{"id":8,"op":"merge"}"#)
        .err()
        .unwrap();
    assert_eq!(
        (id, error.code.as_str()),
        (json!(8), "INVALID_BACKEND_REQUEST")
    );
    let (id, _) = _0_request::decode(r#"{"id":9,"op":"fork","extra":1}"#)
        .err()
        .unwrap();
    assert_eq!(id, json!(9));
    let (id, _) = _0_request::decode("not json").err().unwrap();
    assert_eq!(id, Value::Null);
}

#[test]
fn the_line_server_answers_each_request_and_stops_on_shutdown_or_end_of_input() {
    let input = concat!(
        "{\"id\":1,\"op\":\"fork\"}\n",
        "\n",
        "{\"id\":2,\"branch\":1,\"op\":{\"command\":{\"method\":\"send\",\"params\":{\"transactionBase64\":\"credit:a:2\",\"sigVerify\":false}}}}\n",
        "garbage\n",
        "{\"id\":3,\"branch\":9,\"op\":\"close\"}\n",
        "{\"id\":4,\"op\":\"shutdown\"}\n",
        "{\"id\":5,\"op\":\"fork\"}\n",
    );
    let mut output = Vec::new();
    let mut forks = Forks::new();
    _3_serve::serve_lines(
        std::io::Cursor::new(input),
        &mut output,
        &mut forks,
        &budget(),
        |_| Ok(()),
        None,
        None,
    )
    .unwrap();
    let replies: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 5, "{replies:?}");
    assert_eq!(replies[0], json!({"id":1,"result":{"branch":1}}));
    assert_eq!(replies[1]["result"]["status"], json!("ok"));
    assert_eq!(replies[2]["id"], Value::Null);
    assert_eq!(replies[3]["error"]["code"], json!("UNKNOWN_BRANCH"));
    assert_eq!(replies[4], json!({"id":4,"result":{"shutdown":true}}));

    let mut output = Vec::new();
    _3_serve::serve_lines(
        std::io::Cursor::new(""),
        &mut output,
        &mut Forks::new(),
        &budget(),
        |_| Ok(()),
        None,
        None,
    )
    .unwrap();
    assert!(output.is_empty());
}

#[test]
fn an_unhealthy_worker_ends_the_server_with_its_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let _writer = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (reader, _) = listener.accept().unwrap();
    let error = _3_serve::serve_lines(
        reader,
        Vec::new(),
        &mut Forks::new(),
        &budget(),
        |_| Err(Error::new("WORKER_EXITED", "worker exited")),
        None,
        None,
    )
    .unwrap_err();
    assert_eq!(error.code, "WORKER_EXITED");
}

#[test]
fn the_largest_account_image_fits_one_request_line_and_larger_lines_fail_closed() {
    // A 10 MiB account (the runtime's data limit) as one override, in base64.
    let image = "A".repeat((10 * 1024 * 1024usize).div_ceil(3) * 4);
    let line = json!({"id":1,"op":{"command":{"method":"overrideAccounts","params":{
        "entries":[{"pubkey":"a","lamports":"1","owner":"o","dataBase64":image}]}}}});
    let mut input = serde_json::to_string(&line).unwrap();
    input.push('\n');
    let mut output = Vec::new();
    _3_serve::serve_lines(
        std::io::Cursor::new(input),
        &mut output,
        &mut Forks::new(),
        &budget(),
        |_| Ok(()),
        None,
        None,
    )
    .unwrap();
    let reply: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(reply["result"]["base64Bytes"], json!(image.len()));

    let oversized = format!("{}\n", "x".repeat(21 * 1024 * 1024));
    let error = _3_serve::serve_lines(
        std::io::Cursor::new(oversized),
        Vec::new(),
        &mut Forks::new(),
        &budget(),
        |_| Ok(()),
        None,
        None,
    )
    .unwrap_err();
    assert_eq!(error.code, "BACKEND_INPUT");
}

#[test]
fn a_peek_reports_unloaded_accounts_without_acquiring_them_on_the_named_branch() {
    let input = concat!(
        "{\"id\":1,\"op\":\"fork\"}\n",
        "{\"id\":2,\"branch\":1,\"op\":{\"command\":{\"method\":\"send\",\"params\":{\"transactionBase64\":\"credit:b:3\",\"sigVerify\":false}}}}\n",
        "{\"id\":3,\"op\":{\"peek\":{\"pubkeys\":[\"a\",\"b\"]}}}\n",
        "{\"id\":4,\"branch\":1,\"op\":{\"peek\":{\"pubkeys\":[\"b\"]}}}\n",
    );
    let mut output = Vec::new();
    let mut forks = Forks::new();
    _3_serve::serve_lines(
        std::io::Cursor::new(input),
        &mut output,
        &mut forks,
        &budget(),
        |_| Ok(()),
        None,
        None,
    )
    .unwrap();
    let replies: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies[2]["result"][0]["account"]["lamports"], json!("10"));
    assert_eq!(replies[2]["result"][1], json!({"loaded": false}));
    assert_eq!(replies[3]["result"][0]["account"]["lamports"], json!("3"));
}
