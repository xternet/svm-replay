use super::*;
mod _0_handshake;
mod _1_frames;
mod _2_connections;
mod _3_messages;
#[cfg(test)]
mod tests;

/// Standard clients confirm over WebSocket: signature and slot subscriptions are served.
const MAX_CONNECTIONS: usize = 64;
const MAX_SUBSCRIPTIONS: usize = 1024;
const MAX_FRAME: u64 = 1024 * 1024;

fn failure(message: impl Into<String>) -> Error {
    Error::new("SESSION_WS", message)
}

struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
    open: bool,
}

struct Pending {
    connection: usize,
    subscription: u64,
    /// A one-shot signature subscription; `None` is a slot subscription.
    signature: Option<String>,
}

/// Single-threaded WebSocket side of the fork's RPC: polled by the HTTP loop,
/// never concurrently with a session request.
pub(super) struct Sockets {
    listener: TcpListener,
    connections: Vec<Connection>,
    pending: Vec<Pending>,
    next: u64,
    /// The slot slot subscribers last saw; slots move only by explicit advance.
    last_slot: Option<u64>,
}
