use super::*;

pub(in super::super) struct Nonblocking {
    pub(super) fd: i32,
    pub(super) flags: i32,
}
impl Nonblocking {
    pub(in super::super) fn new(fd: i32) -> Result<Self, Error> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io_error("set nonblocking descriptor"));
        }
        Ok(Self { fd, flags })
    }
}
impl Drop for Nonblocking {
    fn drop(&mut self) {
        if unsafe { libc::fcntl(self.fd, libc::F_SETFL, self.flags) } < 0 {
            eprintln!(
                "CLI_IO: restore descriptor {}: {}",
                self.fd,
                io::Error::last_os_error()
            );
        }
    }
}
pub(in super::super) fn io_error(context: &str) -> Error {
    Error::new(
        "CLI_IO",
        format!("{context}: {}", io::Error::last_os_error()),
    )
}

#[derive(Default)]
pub(in super::super) struct Output {
    pub(in super::super) lines: VecDeque<Vec<u8>>,
    pub(in super::super) offset: usize,
    pub(in super::super) queued: usize,
    pub(in super::super) total: usize,
}
impl Output {
    pub(in super::super) fn push(&mut self, value: &Value) -> Result<(), Error> {
        self.push_bounded(value, MAX_STREAM)
    }
    pub(in super::super) fn push_bounded(
        &mut self,
        value: &Value,
        limit: usize,
    ) -> Result<(), Error> {
        let mut line = crate::json_output::encode(value)?;
        line.push(b'\n');
        self.total = self
            .total
            .checked_add(line.len())
            .ok_or_else(|| Error::new("CLI_OUTPUT_LIMIT", "output accounting overflow"))?;
        if self.total > limit {
            return Err(Error::new(
                "CLI_OUTPUT_LIMIT",
                "JSON output exceeds configured bound; use --out for large exports",
            ));
        }
        self.queued += line.len();
        self.lines.push_back(line);
        Ok(())
    }
    pub(in super::super) fn flush(&mut self, fd: i32) -> Result<(), Error> {
        while let Some(line) = self.lines.front() {
            let remaining = &line[self.offset..];
            let count = unsafe { libc::write(fd, remaining.as_ptr().cast(), remaining.len()) };
            if count < 0 {
                match io::Error::last_os_error().kind() {
                    io::ErrorKind::WouldBlock => return Ok(()),
                    io::ErrorKind::Interrupted => continue,
                    _ => return Err(io_error("write JSONL output")),
                }
            }
            if count == 0 {
                return Err(Error::new("CLI_IO", "zero-length JSONL write"));
            }
            self.offset += count as usize;
            self.queued -= count as usize;
            if self.offset == line.len() {
                self.lines.pop_front();
                self.offset = 0;
            }
        }
        Ok(())
    }
}
