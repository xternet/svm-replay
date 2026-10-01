use super::*;

#[cfg(any(unix, windows))]
pub(in super::super) const MAX_LINE: usize = 64 * 1024;

#[cfg(any(unix, windows))]
pub(in super::super) const MAX_STREAM: usize = 16 * 1024 * 1024 + 32768;

#[cfg(any(unix, windows))]
#[derive(Default)]
pub(in super::super) struct Input {
    pub(in super::super) line: Vec<u8>,
    pub(in super::super) total: usize,
    pub(in super::super) commands: usize,
}

#[cfg(any(unix, windows))]
impl Input {
    pub(in super::super) fn feed(
        &mut self,
        bytes: &[u8],
        controller: &DebugController,
    ) -> Result<(), Error> {
        self.total = self
            .total
            .checked_add(bytes.len())
            .ok_or_else(|| Error::new("DEBUG_INPUT_LIMIT", "input accounting overflow"))?;
        if self.total > MAX_STREAM {
            return Err(Error::new(
                "DEBUG_INPUT_LIMIT",
                "command stream exceeds 16 MiB plus framing",
            ));
        }
        for byte in bytes {
            if *byte == b'\n' {
                if self.line.last() == Some(&b'\r') {
                    self.line.pop();
                }
                let value = svm_replay_protocol::parse_json(&self.line)
                    .map_err(|e| Error::new("DEBUG_INPUT", e.to_string()))?;
                let command: DebugCommand = serde_json::from_value(value)
                    .map_err(|e| Error::new("DEBUG_INPUT", e.to_string()))?;
                self.commands += 1;
                if self.commands > 32768 {
                    return Err(Error::new("DEBUG_INPUT_LIMIT", "too many debug commands"));
                }
                controller.send(command)?;
                self.line.clear();
            } else {
                if self.line.len() == MAX_LINE {
                    return Err(Error::new(
                        "DEBUG_INPUT_LIMIT",
                        "debug command line exceeds 64 KiB",
                    ));
                }
                self.line.push(*byte);
            }
        }
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
pub fn run(_: PreparedRequest, _: &Config, _: CancellationToken) -> Result<Value, Error> {
    Err(Error::new(
        "UNSUPPORTED_PLATFORM",
        "CLI live debugging is qualified only on Linux",
    ))
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn write_final(_: &Value) -> Result<(), Error> {
    Err(Error::new(
        "UNSUPPORTED_PLATFORM",
        "bounded CLI output is qualified only on Linux",
    ))
}
