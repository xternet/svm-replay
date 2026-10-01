use super::*;

pub(in super::super) fn worker_identity(worker: &WorkerDescriptor) -> Result<Value, Error> {
    let mut worker = worker.clone();
    worker.capabilities.sort();
    check(
        worker
            .capabilities
            .windows(2)
            .all(|pair| pair[0] != pair[1]),
        "duplicate worker capabilities",
    )?;
    for name in [&worker.family, &worker.executor_source_id]
        .into_iter()
        .chain(worker.capabilities.iter())
    {
        check(
            !name.is_empty()
                && name.len() <= 200
                && name.as_bytes()[0].is_ascii_alphanumeric()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte)),
            "invalid worker identifier",
        )?;
    }
    serde_json::to_value(worker).map_err(|error| Error::new("CAPTURE_FORMAT", error.to_string()))
}

pub fn validate_capability(
    request: &CaptureRequest,
    pinned: &WorkerDescriptor,
    installed: &WorkerDescriptor,
) -> Result<(), Error> {
    request.validate()?;
    if worker_identity(pinned)? != worker_identity(installed)? {
        return Err(Error::new(
            "WORKER_IDENTITY",
            "capture worker differs from exact pin",
        ));
    }
    let mut required = Vec::new();
    if request.level != "off" {
        required.push("runtime-call-journal/v1");
    }
    if request.execution_mode == "interpreter-debug" {
        required.push("runtime-interpreter-debug/v1");
    }
    if request.execution_mode == "interpreter" {
        required.push("runtime-interpreter-capture/v1");
    }
    if request.level == "sbpf" {
        required.push("runtime-sbpf-trace/v1");
    }
    if request.sbpf_observations == "pc-registers-memory" {
        required.push("runtime-sbpf-memory/v1");
    }
    for capability in required {
        if !installed
            .capabilities
            .iter()
            .any(|value| value == capability)
        {
            return Err(Error::new(
                "CAPABILITY_UNAVAILABLE",
                format!("{} lacks {capability}", installed.family),
            ));
        }
    }
    Ok(())
}

pub fn capture_identity(
    input: &Digest,
    worker: &WorkerDescriptor,
    request: &CaptureRequest,
    bounds: &ProducerBounds,
) -> Result<Digest, Error> {
    bounds.validate(request)?;
    let mut request = request.clone();
    request.filter.program_ids.sort();
    request.filter.instruction_indices.sort();
    Ok(Digest::of(canonical_json(
        &json!({"schema":"svm-m17-capture-identity/v1","inputSha256":input,
        "worker":worker_identity(worker)?,"capture":request,"producerBounds":bounds}),
    )))
}
