use super::*;

pub(in super::super) fn encoded(value: &impl Serialize) -> Result<Vec<u8>, Error> {
    let value = serde_json::to_value(value).map_err(|e| error("SESSION_FORMAT", e.to_string()))?;
    Ok(canonical_json(&value).into_bytes())
}

pub(in super::super) fn control(request: &PreparedRequest) -> Result<PreparedRequest, Error> {
    let mut request = request.clone();
    let fixture = request
        .fixture
        .as_object_mut()
        .ok_or_else(|| error("SESSION_FORMAT", "fixture is not an object"))?;
    let target = fixture
        .get_mut("target")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| error("SESSION_FORMAT", "fixture target missing"))?;
    require(
        target.contains_key("replacementTransactionBase64")
            && fixture.contains_key("accountOverride"),
        "SESSION_FORMAT",
        "explicit variant fields missing",
    )?;
    fixture
        .get_mut("target")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| error("SESSION_FORMAT", "target disappeared"))?
        .insert("replacementTransactionBase64".into(), Value::Null);
    fixture.insert("accountOverride".into(), Value::Null);
    fixture.remove("requestedAccountOverrides");
    Ok(request)
}

pub(in super::super) fn policy(manifest: &SessionManifest) -> Result<(), Error> {
    require(
        manifest.schema == "svm-m17-saved-debug-session/v1" && manifest.assurance == ASSURANCE,
        "SESSION_FORMAT",
        "unknown saved session schema or assurance",
    )?;
    require(
        manifest.reference.file == "reference-worker"
            && manifest.capture.worker.file == "capture-worker",
        "SESSION_FORMAT",
        "unexpected bundled worker names",
    )?;
    require(
        manifest.capture.reference_sha256 == manifest.reference.sha256
            && manifest.capture.worker.family == manifest.reference.family
            && manifest.capture.worker.executor_source_id == manifest.reference.executor_source_id,
        "SESSION_IDENTITY",
        "capture and baseline family/source/gate reference differ",
    )?;
    validate_capability(
        &manifest.policy,
        &manifest.capture.worker,
        &manifest.capture.worker,
    )?;
    manifest.bounds.validate(&manifest.policy)?;
    require(
        manifest.policy.level != "off" || manifest.policy.execution_mode != "interpreter-debug",
        "SESSION_FORMAT",
        "off interpreter capture has no debugger listener",
    )?;
    let newest = manifest.capture.worker.family == "v4-2"
        && manifest.capture.worker.executor_source_id == "litesvm-v0.16.0-agave-4.2.1";
    require(
        newest
            || manifest
                .capture
                .worker
                .capabilities
                .iter()
                .any(|c| c == "runtime-full-capture-cli/v1"),
        "CAPABILITY_UNAVAILABLE",
        "saved role lacks full capture CLI adapter",
    )?;
    require(
        newest || manifest.bounds.memory_rows.is_none(),
        "CAPABILITY_UNAVAILABLE",
        "legacy capture worker has no memory trace adapter",
    )?;
    for worker in [&manifest.reference, &manifest.capture.worker] {
        require(
            !worker.executor_source_id.is_empty()
                && !worker.capabilities.is_empty()
                && worker.capabilities.len()
                    == worker.capabilities.iter().collect::<BTreeSet<_>>().len(),
            "SESSION_IDENTITY",
            "invalid worker capabilities/source identity",
        )?;
    }
    if let Some(lineage) = &manifest.lineage {
        require(
            lineage.kind == "hypothetical",
            "SESSION_FORMAT",
            "unknown branch lineage",
        )?;
    }
    Ok(())
}

pub(in super::super) fn session_identity(manifest: &SessionManifest) -> Result<Digest, Error> {
    let mut value =
        serde_json::to_value(manifest).map_err(|e| error("SESSION_FORMAT", e.to_string()))?;
    value
        .as_object_mut()
        .ok_or_else(|| error("SESSION_FORMAT", "manifest is not object"))?
        .remove("sessionIdentity");
    Ok(Digest::of(canonical_json(&value)))
}

pub(in super::super) fn validate_request(
    request: &PreparedRequest,
    reference: &WorkerDescriptor,
) -> Result<(), Error> {
    request.validate()?;
    require(
        request.family == reference.family,
        "SESSION_IDENTITY",
        "prepared runtime family differs",
    )?;
    let binding = request
        .fixture
        .pointer("/runtime/binding")
        .ok_or_else(|| error("SESSION_IDENTITY", "runtime binding missing"))?;
    svm_replay_protocol::runtime::validate_runtime_binding(binding)?;
    let executor = &binding["executor"];
    let build = &executor["m9Build"];
    for (actual, expected) in [
        (&executor["id"], reference.executor_source_id.as_str()),
        (
            &request.candidate["executorSourceId"],
            reference.executor_source_id.as_str(),
        ),
        (
            &executor["sourceEvidenceHash"],
            reference.source_sha256.as_str(),
        ),
        (&build["binarySha256"], reference.sha256.as_str()),
        (&build["buildHash"], reference.build_hash.as_str()),
    ] {
        require(
            actual.as_str() == Some(expected),
            "SESSION_IDENTITY",
            "prepared and pinned worker provenance differ",
        )?;
    }
    let capabilities: Vec<String> = serde_json::from_value(build["capabilities"].clone())
        .map_err(|e| error("SESSION_IDENTITY", e.to_string()))?;
    require(
        capabilities.len() == capabilities.iter().collect::<BTreeSet<_>>().len()
            && capabilities.iter().collect::<BTreeSet<_>>()
                == reference.capabilities.iter().collect(),
        "SESSION_IDENTITY",
        "prepared worker capabilities differ",
    )
}
