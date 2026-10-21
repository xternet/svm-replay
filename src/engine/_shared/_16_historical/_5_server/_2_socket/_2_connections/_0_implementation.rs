use super::*;

/// Accepting, reading and closing connections; messages are answered in `_3_messages`.
impl Sockets {
    pub fn new(listener: TcpListener) -> Result<Self, Error> {
        listener
            .set_nonblocking(true)
            .map_err(|e| failure(e.to_string()))?;
        Ok(Self {
            listener,
            connections: Vec::new(),
            pending: Vec::new(),
            last_slot: None,
            next: 1,
        })
    }

    /// Accept and answer whatever is ready, then deliver resolved signatures.
    pub fn poll(
        &mut self,
        session: &mut VerifiedSession,
        budget: &ExecutionBudget,
        changed: bool,
    ) -> Result<(), Error> {
        while let Ok((mut stream, peer)) = self.listener.accept() {
            let ready = stream
                .set_nonblocking(false)
                .and_then(|()| stream.set_read_timeout(Some(Duration::from_millis(25))))
                .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(5))))
                .map_err(|e| failure(e.to_string()))
                .and_then(|()| {
                    budget
                        .narrowed(Duration::from_secs(5))
                        .map_err(protocol_error)
                })
                .and_then(|call| _0_handshake::accept(&mut stream, &call))
                .and_then(|()| {
                    stream
                        .set_nonblocking(true)
                        .map_err(|e| failure(e.to_string()))
                });
            match ready {
                Ok(()) if self.connections.len() < MAX_CONNECTIONS => {
                    self.connections.push(Connection {
                        stream,
                        buffer: Vec::new(),
                        open: true,
                    })
                }
                Ok(()) => eprintln!("historical WebSocket {peer}: too many connections"),
                Err(error) => eprintln!("historical WebSocket {peer}: {}", error.message),
            }
        }
        let mut changed = changed;
        for index in 0..self.connections.len() {
            changed |= self.read(index);
        }
        if changed && !self.pending.is_empty() {
            self.deliver(session, budget)?;
        }
        Ok(())
    }

    fn read(&mut self, index: usize) -> bool {
        let mut chunk = [0_u8; 8192];
        loop {
            match self.connections[index].stream.read(&mut chunk) {
                Ok(0) => {
                    self.connections[index].open = false;
                    break;
                }
                Ok(count) => self.connections[index]
                    .buffer
                    .extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => {
                    self.connections[index].open &= error.kind() == std::io::ErrorKind::WouldBlock;
                    break;
                }
            }
        }
        let mut subscribed = false;
        while self.connections[index].open {
            match _1_frames::parse(&mut self.connections[index].buffer) {
                Ok(None) => break,
                Ok(Some(_1_frames::Frame::Text(text))) => subscribed |= self.message(index, &text),
                Ok(Some(_1_frames::Frame::Ping(payload))) => {
                    let pong = _1_frames::encode(10, &payload);
                    let connection = &mut self.connections[index];
                    connection.open &= connection.stream.write_all(&pong).is_ok();
                }
                Ok(Some(_1_frames::Frame::Pong)) => {}
                Ok(Some(_1_frames::Frame::Close(code))) => {
                    let connection = &mut self.connections[index];
                    let _ = connection.stream.write_all(&_1_frames::encode(8, &code));
                    connection.open = false;
                }
                Err(error) => {
                    // 1002: protocol error.
                    let connection = &mut self.connections[index];
                    let _ = connection
                        .stream
                        .write_all(&_1_frames::encode(8, &1002_u16.to_be_bytes()));
                    connection.open = false;
                    eprintln!("historical WebSocket: {}", error.message);
                }
            }
        }
        if !self.connections[index].open {
            self.pending.retain(|pending| pending.connection != index);
        }
        subscribed
    }

    /// Normal closure (1000) when the fork closes: clients reconnect after any other code.
    pub fn shutdown(&mut self) {
        for connection in &mut self.connections {
            let _ = connection
                .stream
                .write_all(&_1_frames::encode(8, &1000_u16.to_be_bytes()));
        }
        self.connections.clear();
        self.pending.clear();
    }

    /// Drop closed connections; indexes in `pending` stay valid until then.
    pub fn sweep(&mut self) {
        if self.connections.iter().all(|c| c.open) {
            return;
        }
        let mut map = Vec::with_capacity(self.connections.len());
        let mut kept = 0;
        for connection in &self.connections {
            map.push(connection.open.then_some(kept));
            kept += usize::from(connection.open);
        }
        self.connections.retain(|c| c.open);
        self.pending.retain_mut(|p| match map[p.connection] {
            Some(index) => {
                p.connection = index;
                true
            }
            None => false,
        });
    }
}
