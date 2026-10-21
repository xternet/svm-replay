use super::*;
use crate::shared::runtime::CancellationToken;

fn read_request(bytes: Vec<u8>) -> Result<Vec<u8>, Error> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let writer = std::thread::spawn(move || {
        let mut client = TcpStream::connect(address).unwrap();
        client.write_all(&bytes).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
    });
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(25)))
        .unwrap();
    let budget = ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).unwrap();
    let result = _0_wire::read(&mut stream, &budget);
    writer.join().unwrap();
    result
}

#[test]
fn http_framing_accepts_only_bounded_unambiguous_json_posts() {
    let body = b"{\"method\":\"getSlot\"}";
    let mut request = format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
    request.extend_from_slice(body);
    assert_eq!(read_request(request).unwrap(), body);
    for request in [
        "POST / HTTP/1.1\r\nContent-Length: 2\r\nContent-Length: 2\r\nContent-Type: application/json\r\n\r\n{}",
        "POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\n\r\n0\r\n\r\n",
        "POST / HTTP/1.1\r\nContent-Length: 16777217\r\nContent-Type: application/json\r\n\r\n",
        "POST / HTTP/1.1\r\nContent-Length: 2\r\nContent-Type: text/plain\r\n\r\n{}",
        "GET / HTTP/1.1\r\n\r\n",
        "POST / HTTP/1.1\r\nContent-Length: 3\r\nContent-Type: application/json\r\n\r\n{}",
    ] { assert!(read_request(request.as_bytes().to_vec()).is_err(), "{request}"); }
}
