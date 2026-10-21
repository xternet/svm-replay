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

#[derive(Clone)]
pub struct ExecutionBudget {
    pub(super) deadline: Instant,
    pub(super) cancellation: CancellationToken,
}

mod _0_implementation;
