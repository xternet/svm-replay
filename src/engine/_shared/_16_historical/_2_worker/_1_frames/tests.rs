use super::*;
use crate::shared::runtime::{CancellationToken, WorkerErrorCode};

fn budget() -> ExecutionBudget {
    ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).unwrap()
}

#[test]
fn partial_frames_remain_buffered_and_eof_is_not_success() {
    let (client, mut server) = UnixStream::pair().unwrap();
    let mut wire = Wire::new(client, 128).unwrap();
    let producer = std::thread::spawn(move || {
        server.write_all(b"{\"a\":").unwrap();
        std::thread::sleep(Duration::from_millis(80));
        server.write_all(b"1}\n{\"b\":2}\npartial").unwrap();
    });
    let base = budget();
    assert_eq!(wire.read(&base, &base).unwrap(), serde_json::json!({"a":1}));
    assert_eq!(wire.read(&base, &base).unwrap(), serde_json::json!({"b":2}));
    assert!(wire
        .read(&base, &base)
        .unwrap_err()
        .message
        .contains("complete frame"));
    producer.join().unwrap();
}

#[test]
fn bounded_response_and_cancelled_partial_read_fail() {
    let (client, mut server) = UnixStream::pair().unwrap();
    let mut wire = Wire::new(client, 5).unwrap();
    let base = budget();
    server.write_all(b"12345").unwrap();
    assert!(wire
        .read(&base, &base)
        .unwrap_err()
        .message
        .contains("limit"));
    let (client, _server) = UnixStream::pair().unwrap();
    let mut wire = Wire::new(client, 128).unwrap();
    let call = base.narrowed(Duration::from_millis(40)).unwrap();
    assert_eq!(wire.read(&base, &call).unwrap_err().code, "WORKER_TIMEOUT");
    let cloned = base.clone();
    cloned.cancel();
    assert_eq!(base.check().unwrap_err().code, WorkerErrorCode::Cancelled);
    assert_eq!(
        wire.read(&base, &base).unwrap_err().code,
        "WORKER_CANCELLED"
    );
}
