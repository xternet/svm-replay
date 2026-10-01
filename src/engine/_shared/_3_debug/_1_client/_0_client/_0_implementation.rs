use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DebugState {
    Stopped,
    Running,
    VmExited,
    VmTerminated,
    Cancelled,
    Disconnected,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Stop {
    Stopped { signal: u8 },
    VmExited { code: u8 },
    VmTerminated { signal: u8 },
}

pub(in super::super) enum PendingKind {
    Text,
    Monitor,
    Advance { command: char, drain_delayed: bool },
}

pub(in super::super) struct Pending {
    pub(in super::super) kind: PendingKind,
    pub(in super::super) budget: ExecutionBudget,
    pub(in super::super) console: Vec<u8>,
    pub(in super::super) wire_bytes: usize,
}

pub(in super::super) enum Reply {
    Text(String),
    Stop(Stop),
}

pub struct DebugClient {
    pub(in super::super) stream: TcpStream,
    pub(in super::super) state: DebugState,
    pub(in super::super) maximum: usize,
    pub(in super::super) budget: ExecutionBudget,
    pub(in super::super) input: Vec<u8>,
    pub(in super::super) pending: Option<Pending>,
    pub(in super::super) pause_pending: bool,
    pub(in super::super) delayed_interrupt: bool,
    pub(in super::super) breakpoints: BTreeSet<u64>,
}

pub(in super::super) fn io_error(phase: &str, e: io::Error) -> Error {
    error("DEBUG_TRANSPORT", format!("{phase}: {e}"))
}

pub(in super::super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(in super::super) fn unhex(value: &str) -> Result<Vec<u8>, Error> {
    require(
        value.len() % 2 == 0 && value.bytes().all(|b| b.is_ascii_hexdigit()),
        "DEBUG_PROTOCOL",
        "malformed hexadecimal response",
    )?;
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|b| {
            u8::from_str_radix(
                std::str::from_utf8(b).map_err(|e| error("DEBUG_PROTOCOL", e.to_string()))?,
                16,
            )
            .map_err(|e| error("DEBUG_PROTOCOL", e.to_string()))
        })
        .collect()
}
impl DebugClient {
    pub fn connect(port: u16, maximum: usize, budget: &ExecutionBudget) -> Result<Self, Error> {
        require(
            port > 0 && (64..=1024 * 1024).contains(&maximum),
            "DEBUG_CONFIG",
            "invalid loopback endpoint or packet limit",
        )?;
        budget.check().map_err(protocol_error)?;
        let stream = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            Duration::from_millis(100),
        )
        .map_err(|e| io_error("attach loopback debugger", e))?;
        stream
            .set_nonblocking(true)
            .map_err(|e| io_error("set nonblocking", e))?;
        stream
            .set_nodelay(true)
            .map_err(|e| io_error("set nodelay", e))?;
        Ok(Self {
            stream,
            state: DebugState::Stopped,
            maximum,
            budget: budget
                .narrowed(Duration::from_secs(300))
                .map_err(protocol_error)?,
            input: Vec::new(),
            pending: None,
            pause_pending: false,
            delayed_interrupt: false,
            breakpoints: BTreeSet::new(),
        })
    }
}
impl DebugClient {
    pub fn state(&self) -> DebugState {
        self.state
    }
}
impl DebugClient {
    pub(in super::super) fn stopped(&self) -> Result<(), Error> {
        require(
            self.state == DebugState::Stopped,
            "DEBUG_STATE",
            "operation requires a paused VM",
        )
    }
}
impl DebugClient {
    pub(in super::super) fn write(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let deadline = self
            .budget
            .narrowed(Duration::from_secs(10))
            .map_err(protocol_error)?;
        let mut position = 0;
        while position < bytes.len() {
            deadline.check().map_err(protocol_error)?;
            match self.stream.write(&bytes[position..]) {
                Ok(0) => return Err(error("DEBUG_DISCONNECTED", "socket closed during write")),
                Ok(size) => position += size,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1))
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_error("write RSP", e)),
            }
        }
        Ok(())
    }
}
impl DebugClient {
    pub(in super::super) fn begin(
        &mut self,
        command: &str,
        kind: PendingKind,
    ) -> Result<(), Error> {
        require(
            self.pending.is_none(),
            "DEBUG_STATE",
            "connection already has an outstanding command",
        )?;
        require(
            command.len() <= self.maximum,
            "DEBUG_LIMIT",
            "command exceeds packet bound",
        )?;
        self.pending = Some(Pending {
            kind,
            budget: self
                .budget
                .narrowed(Duration::from_secs(10))
                .map_err(protocol_error)?,
            console: Vec::new(),
            wire_bytes: 0,
        });
        let sum = command.bytes().fold(0_u8, |sum, b| sum.wrapping_add(b));
        self.write(format!("${command}#{sum:02x}").as_bytes())
    }
}
