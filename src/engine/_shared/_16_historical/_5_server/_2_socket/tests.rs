use super::*;
use crate::shared::runtime::CancellationToken;

fn handshake(request: &'static str) -> (Result<(), Error>, String) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let client = std::thread::spawn(move || {
        let mut client = TcpStream::connect(address).unwrap();
        client.write_all(request.as_bytes()).unwrap();
        let mut reply = String::new();
        let _ = client.read_to_string(&mut reply);
        reply
    });
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(25)))
        .unwrap();
    let budget = ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).unwrap();
    let result = _0_handshake::accept(&mut stream, &budget);
    drop(stream);
    (result, client.join().unwrap())
}

#[test]
fn websocket_handshake_answers_rfc6455_key_and_refuses_browsers_and_plain_http() {
    let (result, reply) = handshake("GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n");
    result.unwrap();
    assert!(reply.starts_with("HTTP/1.1 101 "), "{reply}");
    assert!(
        reply.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"),
        "{reply}"
    );
    for request in [
        "GET / HTTP/1.1\r\nUpgrade: websocket\r\nOrigin: https://example.com\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        "GET / HTTP/1.1\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        "POST / HTTP/1.1\r\nUpgrade: websocket\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
    ] {
        assert!(handshake(request).0.is_err(), "{request}");
    }
}

fn masked(head: u8, payload: &[u8]) -> Vec<u8> {
    let mask = [1, 2, 3, 4];
    let mut frame = vec![head];
    if payload.len() < 126 {
        frame.push(0x80 | payload.len() as u8);
    } else {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    }
    frame.extend_from_slice(&mask);
    frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    frame
}

#[test]
fn websocket_frames_parse_masked_text_wait_for_partial_input_and_refuse_the_rest() {
    let long = "x".repeat(300);
    let mut buffer = masked(0x81, b"{\"id\":1}");
    buffer.extend(masked(0x89, b"hi"));
    buffer.extend(masked(0x88, &[0x03, 0xe8, b'b', b'y', b'e']));
    buffer.extend(masked(0x81, long.as_bytes()));
    assert!(
        matches!(_1_frames::parse(&mut buffer).unwrap(), Some(_1_frames::Frame::Text(t)) if t == "{\"id\":1}")
    );
    assert!(
        matches!(_1_frames::parse(&mut buffer).unwrap(), Some(_1_frames::Frame::Ping(p)) if p == b"hi")
    );
    // A normal client close (1000) must be echoed, or clients reconnect forever.
    assert!(
        matches!(_1_frames::parse(&mut buffer).unwrap(), Some(_1_frames::Frame::Close(c)) if c == [0x03, 0xe8])
    );
    let tail = buffer.split_off(buffer.len() - 5);
    assert!(
        _1_frames::parse(&mut buffer).unwrap().is_none(),
        "incomplete frame waits"
    );
    buffer.extend(tail);
    assert!(
        matches!(_1_frames::parse(&mut buffer).unwrap(), Some(_1_frames::Frame::Text(t)) if t == long)
    );
    assert!(buffer.is_empty());
    for frame in [
        vec![0x81, 0x02, b'h', b'i'],
        masked(0x01, b"part"),
        masked(0x82, b"bin"),
    ] {
        assert!(_1_frames::parse(&mut frame.clone()).is_err());
    }
    let encoded = _1_frames::encode(1, long.as_bytes());
    assert_eq!(&encoded[..4], &[0x81, 126, 1, 44]);
    assert_eq!(&encoded[4..], long.as_bytes());
}
