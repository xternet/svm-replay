use super::*;

pub struct NativeSession {
    _private: (),
}

impl NativeSession {
    pub fn check(&mut self) -> Result<(), Error> {
        Err(Error::new(
            "UNSUPPORTED_PLATFORM",
            "historical command sockets require Unix",
        ))
    }
    pub fn open(
        _transport: WorkerTransport,
        _worker: WorkerSpec,
        _fixture: Value,
        _after: bool,
        _limits: WorkerLimits,
        _budget: ExecutionBudget,
    ) -> Result<Self, Error> {
        Err(Error::new(
            "UNSUPPORTED_PLATFORM",
            "historical command sockets require Unix",
        ))
    }
    pub fn ready(&self) -> &Value {
        unreachable!("unsupported platforms cannot open sessions")
    }
    pub fn request(
        &mut self,
        _command: SessionCommand,
        _call: &ExecutionBudget,
    ) -> Result<Value, Error> {
        Err(Error::new(
            "UNSUPPORTED_PLATFORM",
            "historical command sockets require Unix",
        ))
    }
    pub fn close(self) -> Result<WorkerOutput, Error> {
        Err(Error::new(
            "UNSUPPORTED_PLATFORM",
            "historical command sockets require Unix",
        ))
    }
}
