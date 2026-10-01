use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
};
fn loopback(
    status: u16,
    body: Vec<u8>,
    delay: bool,
) -> (HttpsTransport, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let address = listener.local_addr().expect("loopback address");
    let task = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("synthetic request");
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("read bound");
        let mut headers = Vec::new();
        let mut byte = [0];
        while !headers.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).expect("request headers");
            headers.push(byte[0]);
            assert!(headers.len() < 8192);
        }
        // Drain the POST body before closing: Windows resets sockets closed
        // with unread input, hiding the response from the real HTTP client.
        let length: usize = std::str::from_utf8(&headers)
            .unwrap()
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().unwrap())
            })
            .expect("request content length");
        assert!(length < 8192);
        socket
            .read_exact(&mut vec![0; length])
            .expect("request body");
        if delay {
            std::thread::sleep(Duration::from_millis(50));
            return;
        }
        let mut response = format!(
            "HTTP/1.1 {status} Synthetic\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend(body);
        socket.write_all(&response).expect("synthetic response");
    });
    // Only this private test builds an HTTP client. The production factory
    // always uses HTTPS and the fixed Alchemy endpoint/environment credential.
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .build()
        .expect("test client");
    (
        HttpsTransport {
            client,
            endpoint: format!("http://{address}/v2/synthetic-secret"),
            credential: "synthetic-secret".into(),
        },
        task,
    )
}
#[test]
fn bounded_reqwest_transport_reads_real_local_http_bytes() {
    let (transport, server) = loopback(200, b"public response".to_vec(), false);
    let response = transport
        .post(b"{}", Duration::from_secs(1), 100)
        .expect("response");
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"public response");
    server.join().expect("server");
}
#[test]
fn bounded_reqwest_transport_rejects_credential_echo_and_large_response() {
    for body in [b"synthetic-secret".to_vec(), vec![b'x'; 101]] {
        let echo = body.len() < 100;
        let (transport, server) = loopback(200, body, false);
        let failure = transport
            .post(b"{}", Duration::from_secs(1), 100)
            .err()
            .expect("rejected response");
        assert!(matches!(
            (echo, failure),
            (true, TransportFailure::CredentialEcho) | (false, TransportFailure::ResponseTooLarge)
        ));
        server.join().expect("server");
    }
}
#[test]
fn bounded_reqwest_transport_enforces_request_timeout() {
    let (transport, server) = loopback(200, vec![], true);
    assert!(matches!(
        transport.post(b"{}", Duration::from_millis(10), 100),
        Err(TransportFailure::Timeout)
    ));
    server.join().expect("server");
}
