use super::*;

pub(crate) fn execute_pinned(
    request: &PreparedRequest,
    worker: &ResolvedWorker,
    context: CacheContext<'_>,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    options: CacheOptions,
    coordinator: &CoordinatorPin,
) -> Result<Value, Error> {
    coordinator.verify_identity(context.implementation_sha256)?;
    execute_inner(
        request,
        worker,
        context,
        limits,
        budget,
        options,
        Some(coordinator),
    )
}
