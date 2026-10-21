use super::*;

pub(in super::super) enum Frame {
    Text(String),
    Ping(Vec<u8>),
    Pong,
    /// The client's status code, which the reply must echo (RFC 6455 5.5.1):
    /// clients treat a close without it (1005) as a dropped connection and reconnect.
    Close(Vec<u8>),
}

/// Take one complete client frame from the front of `buffer`, if it has arrived.
/// RFC 6455: client frames are masked; fragments and binary messages are refused.
pub(in super::super) fn parse(buffer: &mut Vec<u8>) -> Result<Option<Frame>, Error> {
    if buffer.len() < 2 {
        return Ok(None);
    }
    if buffer[1] & 0x80 == 0 {
        return Err(failure("client frames must be masked"));
    }
    let (length, start) = match buffer[1] & 0x7f {
        126 if buffer.len() >= 4 => (u64::from(u16::from_be_bytes([buffer[2], buffer[3]])), 4),
        127 if buffer.len() >= 10 => (
            u64::from_be_bytes(buffer[2..10].try_into().unwrap_or_default()),
            10,
        ),
        126 | 127 => return Ok(None),
        length => (u64::from(length), 2),
    };
    if length > MAX_FRAME {
        return Err(failure("WebSocket frame exceeds 1 MiB"));
    }
    let end = start + 4 + length as usize;
    if buffer.len() < end {
        return Ok(None);
    }
    let mask = [
        buffer[start],
        buffer[start + 1],
        buffer[start + 2],
        buffer[start + 3],
    ];
    let payload: Vec<u8> = buffer[start + 4..end]
        .iter()
        .enumerate()
        .map(|(index, byte)| byte ^ mask[index % 4])
        .collect();
    let head = buffer[0];
    buffer.drain(..end);
    if head & 0x80 == 0 {
        return Err(failure("fragmented WebSocket messages are not accepted"));
    }
    Ok(Some(match head & 0x0f {
        1 => {
            Frame::Text(String::from_utf8(payload).map_err(|_| failure("text frame is not UTF-8"))?)
        }
        8 => Frame::Close(payload.into_iter().take(2).collect()),
        9 => Frame::Ping(payload),
        10 => Frame::Pong,
        _ => return Err(failure("only text WebSocket messages are accepted")),
    }))
}

/// One unmasked server frame.
pub(in super::super) fn encode(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0x80 | opcode];
    match payload.len() {
        length if length < 126 => frame.push(length as u8),
        length if length <= 0xffff => {
            frame.push(126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            frame.push(127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    frame
}
