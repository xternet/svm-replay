use super::*;
impl WorkerTransport {
    pub fn new(scratch_root: impl AsRef<Path>) -> Self {
        Self {
            scratch_root: scratch_root.as_ref().into(),
            owner: None,
        }
    }
}
impl WorkerTransport {
    pub fn with_owner(mut self, owner: ProcessOwner) -> Self {
        self.owner = Some(owner);
        self
    }
}
impl WorkerTransport {
    pub fn run(
        &self,
        worker: &WorkerSpec,
        fixture: &Value,
        flags: &[String],
        limits: &WorkerLimits,
        budget: &ExecutionBudget,
    ) -> Result<WorkerOutput, WorkerError> {
        self.run_with_files(worker, fixture, flags, &[], limits, budget)
    }
}
impl WorkerTransport {
    pub fn run_with_files(
        &self,
        worker: &WorkerSpec,
        fixture: &Value,
        flags: &[String],
        output_files: &[PathBuf],
        limits: &WorkerLimits,
        budget: &ExecutionBudget,
    ) -> Result<WorkerOutput, WorkerError> {
        self.run_observed(worker, fixture, flags, output_files, limits, budget, None)
    }
}
impl WorkerTransport {
    /// Stream bounded stderr chunks to a bounded nonblocking consumer. A full or
    /// disconnected channel aborts execution; diagnostics remain in the result.
    pub fn run_with_diagnostics(
        &self,
        worker: &WorkerSpec,
        fixture: &Value,
        flags: &[String],
        output_files: &[PathBuf],
        limits: &WorkerLimits,
        budget: &ExecutionBudget,
        diagnostics: &std::sync::mpsc::SyncSender<Vec<u8>>,
    ) -> Result<WorkerOutput, WorkerError> {
        self.run_observed(
            worker,
            fixture,
            flags,
            output_files,
            limits,
            budget,
            Some(diagnostics),
        )
    }
}
