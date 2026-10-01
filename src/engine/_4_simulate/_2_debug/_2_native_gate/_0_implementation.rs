use super::*;

pub(in super::super) fn native_gate(
    request: &PreparedRequest,
    reference: &ResolvedWorker,
    capture: &ResolvedCaptureWorker,
    context: &TraceContext<'_>,
    policy: &CaptureRequest,
    bounds: &ProducerBounds,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
) -> Result<(CaptureRequest, Value, Option<Value>, Vec<(String, Value)>), Error> {
    require(
        policy.execution_mode == "interpreter-debug" && policy.level != "off",
        "DEBUG_CONFIG",
        "live debugger requires explicit interpreter-debug capture, not off/JIT",
    )?;
    validate_capability(
        policy,
        &capture.worker.descriptor,
        &capture.worker.descriptor,
    )?;
    bounds.validate(policy)?;
    require(
        capture
            .worker
            .descriptor
            .capabilities
            .iter()
            .any(|c| c == "runtime-interpreter-debug/v1"),
        "CAPABILITY_UNAVAILABLE",
        "pinned capture role does not declare live interpreter debugging",
    )?;
    // The request budget owns the entire job. The fresh native gate and each live
    // worker session have their own tighter capture cap, never a renewed job.
    let mut jit_policy = policy.clone();
    jit_policy.execution_mode = trace::native_execution_mode().into();
    let jit = trace::execute(
        request,
        reference,
        capture,
        TraceContext {
            transport: context.transport,
            scratch_root: context.scratch_root,
            implementation_sha256: context.implementation_sha256,
        },
        &jit_policy,
        bounds,
        limits,
        budget,
    )?;
    let jit_receipt = jit.receipt;
    let mut gate_control = jit_receipt.get("controlEvidence").cloned().ok_or_else(|| {
        Error::new(
            "DEBUG_PROTOCOL",
            "passed native gate lacks original-control evidence",
        )
    })?;
    require(
        gate_control.is_object()
            && gate_control["complete"] == true
            && gate_control["verification"]["status"] == "PASS",
        "DEBUG_PROTOCOL",
        "native original-control evidence is incomplete",
    )?;
    let gate_kind = if jit_policy.execution_mode == "jit" {
        "fresh-jit-reference-gate"
    } else {
        "fresh-interpreter-reference-gate"
    };
    gate_control["provenance"] = json!({"kind":gate_kind,"executionMode":jit_policy.execution_mode,"liveInterpreterParityEstablished":false});
    let control_evidence = Some(gate_control);
    let baseline = jit.baseline_responses;
    // The live export is returned separately; the native export is only needed to
    // establish its complete capture gate and need not be retained twice.
    drop(jit.exports);
    Ok((jit_policy, jit_receipt, control_evidence, baseline))
}
