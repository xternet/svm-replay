use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Upgrade one loopback client: `GET` with a WebSocket key and no browser Origin,
/// matching the HTTP side, which browsers cannot call without CORS.
pub(in super::super) fn accept(
    stream: &mut TcpStream,
    budget: &ExecutionBudget,
) -> Result<(), Error> {
    let mut bytes = Vec::new();
    let end = loop {
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end;
        }
        if bytes.len() >= 16 * 1024 {
            return Err(failure("WebSocket handshake exceeds 16 KiB"));
        }
        budget.check().map_err(protocol_error)?;
        let mut chunk = [0_u8; 2048];
        match stream.read(&mut chunk) {
            Ok(0) => return Err(failure("incomplete WebSocket handshake")),
            Ok(count) => bytes.extend_from_slice(&chunk[..count]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(failure(format!("read WebSocket handshake: {error}"))),
        }
    };
    let header =
        std::str::from_utf8(&bytes[..end]).map_err(|_| failure("invalid handshake encoding"))?;
    let mut lines = header.split("\r\n");
    let request = lines.next().unwrap_or_default();
    if !request.starts_with("GET ") || !request.ends_with(" HTTP/1.1") {
        return Err(failure("WebSocket handshake requires GET over HTTP/1.1"));
    }
    let (mut key, mut upgrade) = (None, false);
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| failure("invalid handshake header"))?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("origin") {
            return Err(failure("browser origins are not accepted"));
        }
        if name.eq_ignore_ascii_case("sec-websocket-key") {
            key = Some(value);
        }
        upgrade |= name.eq_ignore_ascii_case("upgrade") && value.eq_ignore_ascii_case("websocket");
    }
    let key = key
        .filter(|key| upgrade && !key.is_empty())
        .ok_or_else(|| failure("not a WebSocket upgrade"))?;
    let digest = sha1_smol::Sha1::from(format!("{key}{GUID}"))
        .digest()
        .bytes();
    let reply = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        STANDARD.encode(digest)
    );
    stream
        .write_all(reply.as_bytes())
        .map_err(|error| failure(format!("write WebSocket handshake: {error}")))
}
