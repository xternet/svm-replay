use super::*;

impl DebugClient {
    pub(in super::super) fn poll_inner(&mut self) -> Result<Option<Reply>, Error> {
        self.budget.check().map_err(protocol_error)?;
        if let Some(pending) = &self.pending {
            pending.budget.check().map_err(protocol_error)?;
        }
        for _ in 0..16 {
            if let Some(decoded) = self.packet()? {
                let answer = String::from_utf8(decoded)
                    .map_err(|e| error("DEBUG_PROTOCOL", format!("non-UTF8 RSP response: {e}")))?;
                require(
                    !(answer.starts_with('E')
                        && answer.len() >= 3
                        && answer.as_bytes()[1..3]
                            .iter()
                            .all(|b| b.is_ascii_hexdigit())),
                    "DEBUG_TARGET",
                    &format!("target rejected read-only command: {answer}"),
                )?;
                let mut pending = self
                    .pending
                    .take()
                    .ok_or_else(|| error("DEBUG_PROTOCOL", "response without pending command"))?;
                match pending.kind {
                    PendingKind::Monitor if answer.starts_with('O') && answer != "OK" => {
                        pending.console.extend(unhex(&answer[1..])?);
                        self.pending = Some(pending);
                        continue;
                    }
                    PendingKind::Monitor => {
                        require(
                            answer == "OK",
                            "DEBUG_TARGET",
                            "runtime metadata unavailable",
                        )?;
                        return Ok(Some(Reply::Text(
                            String::from_utf8(pending.console)
                                .map_err(|e| error("DEBUG_PROTOCOL", e.to_string()))?,
                        )));
                    }
                    PendingKind::Text => return Ok(Some(Reply::Text(answer))),
                    PendingKind::Advance {
                        command,
                        drain_delayed,
                    } => {
                        require(
                            answer.len() >= 3
                                && answer.as_bytes()[1..3]
                                    .iter()
                                    .all(|b| b.is_ascii_hexdigit()),
                            "DEBUG_PROTOCOL",
                            "malformed execution stop",
                        )?;
                        let code = unhex(&answer[1..3])?[0];
                        let stop = match answer.as_bytes()[0] {
                            b'S' | b'T' => {
                                self.state = DebugState::Stopped;
                                if code == 2 && self.pause_pending {
                                    self.pause_pending = false;
                                    self.delayed_interrupt = false;
                                    if drain_delayed {
                                        self.advance(command)?;
                                        continue;
                                    }
                                } else if self.pause_pending {
                                    self.delayed_interrupt = true;
                                }
                                Stop::Stopped { signal: code }
                            }
                            b'W' if answer.len() == 3 => {
                                self.state = DebugState::VmExited;
                                Stop::VmExited { code }
                            }
                            b'X' if answer.len() == 3 => {
                                self.state = DebugState::VmTerminated;
                                Stop::VmTerminated { signal: code }
                            }
                            _ => {
                                return Err(error(
                                    "DEBUG_PROTOCOL",
                                    format!("unexpected execution stop: {answer}"),
                                ))
                            }
                        };
                        return Ok(Some(Reply::Stop(stop)));
                    }
                }
            }
            let mut chunk = [0_u8; 4096];
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    if self.pending.is_some() {
                        return Err(error("DEBUG_DISCONNECTED", "peer closed before responding"));
                    }
                    return Ok(None);
                }
                Ok(size) => {
                    require(
                        self.input.len() + size <= self.maximum + 8,
                        "DEBUG_LIMIT",
                        "wire packet exceeds bound",
                    )?;
                    self.input.extend_from_slice(&chunk[..size]);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_error("read RSP", e)),
            }
        }
        Ok(None)
    }
}
impl DebugClient {
    pub(in super::super) fn poll(&mut self) -> Result<Option<Reply>, Error> {
        let result = self.poll_inner();
        if let Err(error) = &result {
            // A target-rejected read is explicit but does not resume or damage
            // the stopped VM. Protocol/transport failures remain terminal.
            if error.code != "DEBUG_TARGET" || self.state != DebugState::Stopped {
                self.state = DebugState::Error;
            }
            self.pending = None;
        }
        result
    }
}
impl DebugClient {
    pub(in super::super) fn request(
        &mut self,
        command: &str,
        kind: PendingKind,
    ) -> Result<String, Error> {
        self.stopped()?;
        self.begin(command, kind)?;
        loop {
            match self.poll()? {
                Some(Reply::Text(value)) => return Ok(value),
                Some(Reply::Stop(_)) => {
                    return Err(error("DEBUG_PROTOCOL", "stop in read-only response"))
                }
                None => thread::sleep(Duration::from_millis(1)),
            }
        }
    }
}
impl DebugClient {
    pub fn negotiate(&mut self) -> Result<String, Error> {
        self.request("qSupported:swbreak+", PendingKind::Text)
    }
}
impl DebugClient {
    pub fn metadata(&mut self) -> Result<String, Error> {
        self.request(&format!("qRcmd,{}", hex(b"metadata")), PendingKind::Monitor)
    }
}
impl DebugClient {
    /// The only additional monitor endpoint is read-only actual runtime frames.
    pub fn runtime_frames(&mut self) -> Result<String, Error> {
        self.request(&format!("qRcmd,{}", hex(b"frames")), PendingKind::Monitor)
    }
}
impl DebugClient {
    pub fn registers(&mut self) -> Result<[u64; 12], Error> {
        let bytes = unhex(&self.request("g", PendingKind::Text)?)?;
        require(
            bytes.len() == 96,
            "DEBUG_PROTOCOL",
            "expected twelve complete u64 registers",
        )?;
        let mut registers = [0; 12];
        for (r, b) in registers.iter_mut().zip(bytes.chunks_exact(8)) {
            let mut raw = [0; 8];
            raw.copy_from_slice(b);
            *r = u64::from_le_bytes(raw);
        }
        Ok(registers)
    }
}
impl DebugClient {
    pub fn memory(&mut self, address: u64, length: usize) -> Result<Vec<u8>, Error> {
        require(
            length > 0
                && length <= self.maximum / 2
                && u128::from(address) + length as u128 <= u128::from(u64::MAX) + 1,
            "DEBUG_CONFIG",
            "invalid or oversized memory read",
        )?;
        let bytes = unhex(&self.request(&format!("m{address:x},{length:x}"), PendingKind::Text)?)?;
        require(
            bytes.len() == length,
            "DEBUG_PROTOCOL",
            "memory response length differs",
        )?;
        Ok(bytes)
    }
}
