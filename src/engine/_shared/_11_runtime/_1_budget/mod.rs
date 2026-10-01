use super::*;

#[derive(Clone, Default)]
pub struct CancellationToken(pub(super) Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub struct ExecutionBudget {
    pub(super) deadline: Instant,
    pub(super) cancellation: CancellationToken,
}

impl ExecutionBudget {
    /// Cancel the whole operation and all narrowed budgets sharing its token.
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
    /// Preserve cancellation and the existing deadline while imposing a tighter operation cap.
    pub fn narrowed(&self, timeout: Duration) -> Result<Self, WorkerError> {
        self.check()?;
        let mut child = Self::new(timeout, self.cancellation.clone())?;
        child.deadline = child.deadline.min(self.deadline);
        Ok(child)
    }
    pub fn new(timeout: Duration, cancellation: CancellationToken) -> Result<Self, WorkerError> {
        if timeout.is_zero() {
            return Err(WorkerError::new(
                WorkerErrorCode::InvalidConfig,
                "timeout must be positive",
            ));
        }
        let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
            WorkerError::new(
                WorkerErrorCode::InvalidConfig,
                "timeout exceeds clock range",
            )
        })?;
        Ok(Self {
            deadline,
            cancellation,
        })
    }
    pub fn check(&self) -> Result<(), WorkerError> {
        if self.cancellation.is_cancelled() {
            return Err(WorkerError::new(
                WorkerErrorCode::Cancelled,
                "worker operation cancelled",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(WorkerError::new(
                WorkerErrorCode::Timeout,
                "cumulative operation deadline exceeded",
            ));
        }
        Ok(())
    }
}
