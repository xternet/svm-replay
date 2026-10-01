use super::*;

pub(in super::super) fn admit(
    request: &PreparedRequest,
    worker: &ResolvedWorker,
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
) -> Result<(), Error> {
    budget.check().map_err(protocol_error)?;
    request.validate()?;
    if request.family != worker.descriptor.family {
        return Err(Error::new(
            "WORKER_IDENTITY",
            "cache runtime family differs",
        ));
    }
    if !worker
        .descriptor
        .capabilities
        .iter()
        .any(|value| value == "historical-boundary-checkpoint/v1")
        || !worker
            .descriptor
            .capabilities
            .iter()
            .any(|value| value == "guarded-generic-sysvars/v1")
    {
        return Err(Error::new(
            "UNSUPPORTED_RUNTIME_CAPABILITY",
            "cache requires guarded historical-boundary-checkpoint/v1",
        ));
    }
    let binding = request
        .fixture
        .pointer("/runtime/binding/executor")
        .ok_or_else(|| Error::new("WORKER_IDENTITY", "fixture executor missing"))?;
    let capabilities: Vec<String> =
        serde_json::from_value(field(field(binding, "m9Build")?, "capabilities")?.clone())
            .map_err(|error| Error::new("WORKER_IDENTITY", error.to_string()))?;
    let unique: std::collections::BTreeSet<_> = capabilities.iter().collect();
    if unique.len() != capabilities.len()
        || unique != worker.descriptor.capabilities.iter().collect()
    {
        return Err(Error::new(
            "WORKER_IDENTITY",
            "cache fixture capabilities differ from the pinned worker",
        ));
    }
    for (value, expected) in [
        (
            binding.get("id"),
            worker.descriptor.executor_source_id.as_str(),
        ),
        (
            binding.get("sourceEvidenceHash"),
            worker.descriptor.source_sha256.as_str(),
        ),
        (
            binding.pointer("/m9Build/binarySha256"),
            worker.descriptor.sha256.as_str(),
        ),
        (
            binding.pointer("/m9Build/buildHash"),
            worker.descriptor.build_hash.as_str(),
        ),
    ] {
        if value.and_then(Value::as_str) != Some(expected) {
            return Err(Error::new(
                "WORKER_IDENTITY",
                "cache fixture and worker identity differ",
            ));
        }
    }
    if canonical_json(&request.fixture).len() > limits.max_input_bytes {
        return Err(Error::new(
            "WORKER_INPUTLIMIT",
            "cache fixture exceeds worker input byte limit",
        ));
    }
    Ok(())
}
