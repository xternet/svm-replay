use super::*;

pub(super) fn command() -> Vec<u8> {
    serde_json::to_vec(&json!({"requestId":1,"phase":"original-control","target":null,"action":{"kind":"continue-all"}})).unwrap()
}

pub(super) fn fill(stream: &mut UnixStream) {
    stream.set_nonblocking(true).unwrap();
    loop {
        match stream.write(&[b'x'; 8192]) {
            Ok(count) => assert!(count > 0),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("fill socket: {error}"),
        }
    }
}
