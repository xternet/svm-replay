use super::*;

pub(in super::super::super) fn write(
    stream: &mut TcpStream,
    status: u16,
    value: Option<&Value>,
    budget: &ExecutionBudget,
) -> Result<(), Error> {
    let body = match value {
        Some(value) => serde_json::to_vec(value)
            .map_err(|e| failure(500, &format!("encode RPC response: {e}")))?,
        None => Vec::new(),
    };
    if body.len() > 64 * 1024 * 1024 {
        return Err(failure(500, "RPC response exceeds 64 MiB"));
    }
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        417 => "Expectation Failed",
        _ => "Internal Server Error",
    };
    let header = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n", body.len());
    for bytes in [header.as_bytes(), &body] {
        let mut offset = 0;
        while offset < bytes.len() {
            budget.check().map_err(protocol_error)?;
            match stream.write(&bytes[offset..]) {
                Ok(0) => return Err(failure(500, "RPC response channel closed")),
                Ok(n) => offset += n,
                Err(error) if retry(&error) => continue,
                Err(error) => return Err(failure(500, &format!("write RPC response: {error}"))),
            }
        }
    }
    Ok(())
}
