use super::*;

pub struct DebugExecution {
    pub output: Value,
    pub verification: Value,
    pub control_verification: Value,
    pub exports: Vec<(String, CaptureExport)>,
    pub events: Vec<Value>,
    pub receipt: Value,
    pub control_evidence: Value,
}

pub(in super::super) fn require(condition: bool, code: &str, message: &str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::new(code, message))
    }
}

pub fn execute(
    request: &PreparedRequest,
    reference: &ResolvedWorker,
    capture: &ResolvedCaptureWorker,
    context: TraceContext<'_>,
    policy: &CaptureRequest,
    bounds: &ProducerBounds,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    driver: &mut DebugDriver,
) -> Result<DebugExecution, Error> {
    execute_checked(
        request, reference, capture, context, policy, bounds, limits, budget, driver, None,
    )
}

pub(crate) fn execute_pinned(
    request: &PreparedRequest,
    reference: &ResolvedWorker,
    capture: &ResolvedCaptureWorker,
    context: TraceContext<'_>,
    policy: &CaptureRequest,
    bounds: &ProducerBounds,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    driver: &mut DebugDriver,
    coordinator: &CoordinatorPin,
) -> Result<DebugExecution, Error> {
    coordinator.verify_identity(context.implementation_sha256)?;
    execute_checked(
        request,
        reference,
        capture,
        context,
        policy,
        bounds,
        limits,
        budget,
        driver,
        Some(coordinator),
    )
}

pub(in super::super) fn execute_checked(
    request: &PreparedRequest,
    reference: &ResolvedWorker,
    capture: &ResolvedCaptureWorker,
    context: TraceContext<'_>,
    policy: &CaptureRequest,
    bounds: &ProducerBounds,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    driver: &mut DebugDriver,
    coordinator: Option<&CoordinatorPin>,
) -> Result<DebugExecution, Error> {
    let result = execute_inner(
        request,
        reference,
        capture,
        context,
        policy,
        bounds,
        limits,
        budget,
        driver,
        coordinator,
    );
    if let Err(primary) = &result {
        if let Err(delivery) = driver.failed(primary) {
            return Err(super::super::super::control::attach(
                delivery.with_details(json!({"cause":primary})),
                primary
                    .details
                    .as_ref()
                    .and_then(|details| details.get("controlEvidence")),
            ));
        }
    }
    result
}
