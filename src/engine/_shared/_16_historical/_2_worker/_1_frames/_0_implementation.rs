use super::*;

pub(in super::super) struct Wire {
    stream: UnixStream,
    pending: Vec<u8>,
    scanned: usize,
    limit: usize,
}

impl Wire {
    pub(in super::super) fn new(stream: UnixStream, limit: usize) -> Result<Self, Error> {
        let poll = Some(Duration::from_millis(25));
        stream
            .set_read_timeout(poll)
            .map_err(|e| error(format!("session read timeout: {e}")))?;
        stream
            .set_write_timeout(poll)
            .map_err(|e| error(format!("session write timeout: {e}")))?;
        Ok(Self {
            stream,
            pending: Vec::new(),
            scanned: 0,
            limit,
        })
    }

    pub(in super::super) fn write(
        &mut self,
        value: &Value,
        base: &ExecutionBudget,
        call: &ExecutionBudget,
    ) -> Result<(), Error> {
        let mut bytes = serde_json::to_vec(value).map_err(|e| error(e.to_string()))?;
        bytes.push(b'\n');
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(error("session command exceeds 16 MiB"));
        }
        let mut offset = 0;
        while offset < bytes.len() {
            base.check().map_err(protocol_error)?;
            call.check().map_err(protocol_error)?;
            match self.stream.write(&bytes[offset..]) {
                Ok(0) => return Err(error("session command channel closed")),
                Ok(n) => offset += n,
                Err(e) if retry(&e) => continue,
                Err(e) => return Err(error(format!("write session command: {e}"))),
            }
        }
        Ok(())
    }

    pub(in super::super) fn read(
        &mut self,
        base: &ExecutionBudget,
        call: &ExecutionBudget,
    ) -> Result<Value, Error> {
        loop {
            base.check().map_err(protocol_error)?;
            call.check().map_err(protocol_error)?;
            if let Some(end) = self.pending[self.scanned..]
                .iter()
                .position(|b| *b == b'\n')
            {
                let end = self.scanned + end;
                if end + 1 > self.limit {
                    return Err(error("session response limit exceeded"));
                }
                let value = svm_replay_protocol::parse_json(&self.pending[..end])?;
                base.check().map_err(protocol_error)?;
                call.check().map_err(protocol_error)?;
                self.pending.drain(..=end);
                self.scanned = 0;
                return Ok(value);
            }
            self.scanned = self.pending.len();
            if self.pending.len() >= self.limit {
                return Err(error("session response limit exceeded"));
            }
            let mut bytes = [0_u8; 8192];
            match self.stream.read(&mut bytes) {
                Ok(0) => {
                    return Err(error(
                        "session response channel closed before complete frame",
                    ))
                }
                Ok(n) => self.pending.extend_from_slice(&bytes[..n]),
                Err(e) if retry(&e) => continue,
                Err(e) => return Err(error(format!("read session response: {e}"))),
            }
        }
    }
}

fn retry(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
    )
}
