use super::*;

pub fn resolve(
    catalog: &InstalledCatalog,
    request: &PreparedRequest,
) -> Result<ResolvedWorker, Error> {
    let binding = request
        .fixture
        .pointer("/runtime/binding")
        .ok_or_else(|| problem("WORKER_IDENTITY", "missing runtime binding"))?;
    let worker = resolve_binding(catalog, &request.family, &request.candidate, binding)?;
    data_only_capability(&request.fixture, &worker.descriptor.capabilities)?;
    Ok(worker)
}

pub fn resolve_binding(
    catalog: &InstalledCatalog,
    family: &str,
    candidate: &serde_json::Value,
    binding: &serde_json::Value,
) -> Result<ResolvedWorker, Error> {
    let worker = catalog
        .manifest
        .workers
        .iter()
        .find(|worker| worker.family == family)
        .ok_or_else(|| problem("UNSUPPORTED_RUNTIME", "requested family is not installed"))?;
    validate_binding(family, candidate, binding, worker)?;
    Ok(ResolvedWorker {
        descriptor: worker.clone(),
        spec: WorkerSpec {
            executable: confined(&catalog.root, &worker.file)?,
            sha256: worker.sha256.as_str().into(),
        },
    })
}

pub fn validate_worker_binding(
    request: &PreparedRequest,
    worker: &WorkerDescriptor,
) -> Result<(), Error> {
    let binding = request
        .fixture
        .pointer("/runtime/binding")
        .ok_or_else(|| problem("WORKER_IDENTITY", "missing runtime binding"))?;
    validate_binding(&request.family, &request.candidate, binding, worker)?;
    data_only_capability(&request.fixture, &worker.capabilities)
}

pub(in super::super) fn validate_binding(
    family: &str,
    candidate: &serde_json::Value,
    binding: &serde_json::Value,
    worker: &WorkerDescriptor,
) -> Result<(), Error> {
    svm_replay_protocol::runtime::validate_runtime_binding(binding)?;
    if binding["targetSlot"] != candidate["slot"]
        || binding["runtimeProfileId"] != candidate["runtimeProfileId"]
    {
        return Err(problem(
            "WORKER_IDENTITY",
            "candidate and runtime boundary/profile differ",
        ));
    }
    if family != worker.family {
        return Err(problem(
            "WORKER_IDENTITY",
            "request family differs from selected worker",
        ));
    }
    let executor = binding
        .get("executor")
        .ok_or_else(|| problem("WORKER_IDENTITY", "missing executor"))?;
    let build = executor
        .get("m9Build")
        .ok_or_else(|| problem("WORKER_IDENTITY", "missing reviewed build"))?;
    for (actual, expected) in [
        (executor.get("id"), worker.executor_source_id.as_str()),
        (
            candidate.get("executorSourceId"),
            worker.executor_source_id.as_str(),
        ),
        (
            executor.get("sourceEvidenceHash"),
            worker.source_sha256.as_str(),
        ),
        (build.get("binarySha256"), worker.sha256.as_str()),
        (build.get("buildHash"), worker.build_hash.as_str()),
    ] {
        if actual.and_then(serde_json::Value::as_str) != Some(expected) {
            return Err(problem(
                "WORKER_IDENTITY",
                "request and reviewed catalog identity differ",
            ));
        }
    }
    let actual: Vec<String> = serde_json::from_value(
        build
            .get("capabilities")
            .ok_or_else(|| problem("WORKER_IDENTITY", "missing capabilities"))?
            .clone(),
    )
    .map_err(|e| problem("WORKER_IDENTITY", e.to_string()))?;
    let unique: BTreeSet<_> = actual.iter().collect();
    if unique.len() != actual.len()
        || unique != worker.capabilities.iter().collect()
        || !actual.iter().any(|c| c == "guarded-generic-sysvars/v1")
    {
        return Err(problem(
            "UNSUPPORTED_RUNTIME_CAPABILITY",
            "guarded worker capabilities differ",
        ));
    }
    Ok(())
}

pub fn resolve_capture(
    catalog: &InstalledCatalog,
    request: &PreparedRequest,
) -> Result<ResolvedCaptureWorker, Error> {
    let reference = resolve(catalog, request)?;
    let role = catalog
        .manifest
        .capture_workers
        .iter()
        .find(|role| role.worker.family == request.family)
        .ok_or_else(|| {
            problem(
                "CAPABILITY_UNAVAILABLE",
                "no separately pinned capture worker for requested family",
            )
        })?;
    if role.reference_sha256 != reference.descriptor.sha256
        || role.worker.executor_source_id != reference.descriptor.executor_source_id
    {
        return Err(problem(
            "WORKER_IDENTITY",
            "capture role no longer matches baseline",
        ));
    }
    data_only_capability(&request.fixture, &role.worker.capabilities)?;
    Ok(ResolvedCaptureWorker {
        worker: ResolvedWorker {
            descriptor: role.worker.clone(),
            spec: WorkerSpec {
                executable: confined(&catalog.root, &role.worker.file)?,
                sha256: role.worker.sha256.as_str().into(),
            },
        },
        reference_sha256: role.reference_sha256.clone(),
        gate_sha256: role.gate_sha256.clone(),
    })
}

pub(super) fn data_only_capability(
    fixture: &serde_json::Value,
    capabilities: &[String],
) -> Result<(), Error> {
    if fixture["runtime"].get("slotHashesData").is_some()
        && !capabilities
            .iter()
            .any(|c| c == "proven-slot-hashes-cache/v1")
    {
        return Err(problem(
            "UNSUPPORTED_RUNTIME_CAPABILITY",
            "data-only SlotHashes requires proven-slot-hashes-cache/v1",
        ));
    }
    Ok(())
}
