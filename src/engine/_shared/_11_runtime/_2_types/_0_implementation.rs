use super::*;

#[derive(Clone, Debug)]
pub struct WorkerSpec {
    pub executable: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct ProcessOwner {
    pub executable: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct WorkerLimits {
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_diagnostic_bytes: usize,
    pub cleanup_grace: Duration,
}

impl Default for WorkerLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 256 * 1024 * 1024,
            max_output_bytes: 256 * 1024 * 1024,
            max_diagnostic_bytes: 1024 * 1024,
            cleanup_grace: Duration::from_secs(2),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerErrorCode {
    InvalidConfig,
    UnsupportedPlatform,
    Io,
    Integrity,
    Cancelled,
    Timeout,
    InputLimit,
    OutputLimit,
    DiagnosticLimit,
    WorkerExit,
    InvalidOutput,
    Cleanup,
}

#[derive(Debug)]
pub struct WorkerError {
    pub code: WorkerErrorCode,
    pub message: String,
    pub pid: Option<u32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl WorkerError {
    pub(crate) fn new(code: WorkerErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            pid: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }
    pub(crate) fn io(context: &str, error: io::Error) -> Self {
        Self::new(WorkerErrorCode::Io, format!("{context}: {error}"))
    }
    pub(in super::super) fn diagnostics_from(mut self, other: WorkerError) -> Self {
        self.message = format!(
            "{}; preceding failure {:?}: {}",
            self.message, other.code, other.message
        );
        self.pid = other.pid;
        self.stdout = other.stdout;
        self.stderr = other.stderr;
        self
    }
}

impl fmt::Display for WorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {} (pid {:?})", self.code, self.message, self.pid)
    }
}

impl std::error::Error for WorkerError {}

pub fn protocol_error(error: WorkerError) -> svm_replay_protocol::Error {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    svm_replay_protocol::Error::new(format!("WORKER_{:?}", error.code).to_uppercase(), error.to_string())
        .with_details(serde_json::json!({"pid":error.pid,"stdoutBase64":STANDARD.encode(error.stdout),"stderrBase64":STANDARD.encode(error.stderr)}))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupScope {
    ImmediateWorkerOnly,
    OwnerReapedProcessTree,
    OwnerTerminatedProcessGroup,
    WindowsJobObject,
}

#[derive(Debug)]
pub struct WorkerOutput {
    pub value: Value,
    pub pid: u32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub elapsed: Duration,
    pub cleanup_scope: CleanupScope,
}

pub struct WorkerTransport {
    pub(in super::super) scratch_root: PathBuf,
    pub(in super::super) owner: Option<ProcessOwner>,
}
