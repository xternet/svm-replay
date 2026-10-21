use super::*;

fn receive(
    stream: &mut TcpStream,
    bytes: &mut [u8],
    budget: &ExecutionBudget,
) -> Result<usize, Error> {
    loop {
        budget.check().map_err(protocol_error)?;
        match stream.read(bytes) {
            Ok(0) => return Err(failure(400, "incomplete HTTP request")),
            Ok(n) => return Ok(n),
            Err(error) if retry(&error) => continue,
            Err(error) => return Err(failure(400, &format!("read HTTP request: {error}"))),
        }
    }
}

pub(in super::super::super) fn read(
    stream: &mut TcpStream,
    budget: &ExecutionBudget,
) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    let end = loop {
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            if end + 4 > 16 * 1024 {
                return Err(failure(413, "HTTP headers exceed 16 KiB"));
            }
            break end + 4;
        }
        if bytes.len() >= 16 * 1024 {
            return Err(failure(413, "HTTP headers exceed 16 KiB"));
        }
        let mut chunk = [0_u8; 4096];
        let n = receive(stream, &mut chunk, budget)?;
        bytes.extend_from_slice(&chunk[..n]);
    };
    let header = std::str::from_utf8(&bytes[..end])
        .map_err(|_| failure(400, "invalid HTTP header encoding"))?;
    let mut lines = header.split("\r\n");
    let line = lines
        .next()
        .ok_or_else(|| failure(400, "HTTP request line missing"))?;
    let parts: Vec<_> = line.split_ascii_whitespace().collect();
    if parts.len() != 3 || !["HTTP/1.0", "HTTP/1.1"].contains(&parts[2]) {
        return Err(failure(400, "invalid HTTP request line"));
    }
    if parts[0] != "POST" || parts[1] != "/" {
        return Err(failure(405, "historical RPC requires POST /"));
    }
    let mut length = None;
    let mut content_type = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| failure(400, "invalid HTTP header"))?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(failure(400, "transfer encoding is not accepted"));
        }
        if name.eq_ignore_ascii_case("expect") {
            return Err(failure(417, "Expect is not accepted"));
        }
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(failure(400, "ambiguous or invalid Content-Length"));
            }
            let count = value
                .parse::<usize>()
                .map_err(|_| failure(413, "HTTP body length overflow"))?;
            if count > 16 * 1024 * 1024 {
                return Err(failure(413, "HTTP body exceeds 16 MiB"));
            }
            length = Some(count);
        }
        if name.eq_ignore_ascii_case("content-type") {
            if content_type.is_some() {
                return Err(failure(400, "ambiguous Content-Type"));
            }
            content_type = Some(value);
        }
    }
    let length = length.ok_or_else(|| failure(400, "Content-Length is required"))?;
    if !content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(failure(415, "Content-Type must be application/json"));
    }
    let mut body = bytes[end..].to_vec();
    if body.len() > length {
        return Err(failure(400, "HTTP body exceeds declared Content-Length"));
    }
    while body.len() < length {
        let mut chunk = [0_u8; 8192];
        let wanted = chunk.len().min(length - body.len());
        let count = receive(stream, &mut chunk[..wanted], budget)?;
        body.extend_from_slice(&chunk[..count]);
    }
    Ok(body)
}
