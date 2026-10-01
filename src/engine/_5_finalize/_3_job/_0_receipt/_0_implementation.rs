use super::*;

pub(crate) fn begin_job(
    request: &Value,
    config: &Config,
    scope: &str,
) -> Result<(PathBuf, Value, CoordinatorPin), Error> {
    if let Some(trace) = &config.trace {
        trace.bounds.validate(&trace.capture)?;
    }
    if !config.data_dir.is_absolute() {
        return Err(Error::new(
            "DATA_DIRECTORY",
            "Rust callers must resolve the data directory explicitly",
        ));
    }
    let jobs = crate::shared::layout::runs_dir(&config.data_dir)?;
    let coordinator = CoordinatorPin::current()?;
    ensure_private_directory(&config.data_dir)?;
    ensure_private_directory(&jobs)?;
    let job = tempfile::Builder::new()
        .prefix("job-")
        .tempdir_in(&jobs)
        .map_err(|e| Error::new("DATA_DIRECTORY", e.to_string()))?
        .keep();
    let request_hash = Digest::of(
        serde_json::to_vec(request).map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?,
    );
    let receipt = json!({"schema":"svm-replay-receipt/v1","requestId":request["requestId"],"requestIdentity":request_hash,
        "catalogSha256":config.catalog_sha256,"implementationSha256":coordinator.sha256,"processOwnerSha256":config.owner.sha256,"scope":scope,"currentStateFallback":false,
        "reconstructedFromSources":false,"outcome":"ERROR","phases":[],"controlVerification":null,"verification":null});
    write_json(
        &job.join("started.json"),
        &json!({"schema":"svm-replay-started/v1",
        "requestId":request["requestId"],"requestIdentity":request_hash,"catalogSha256":config.catalog_sha256,
        "implementationSha256":coordinator.sha256,"processOwnerSha256":config.owner.sha256}),
    )?;
    Ok((job, receipt, coordinator))
}

pub(crate) fn finish_job(
    result: Result<Value, Error>,
    mut receipt: Value,
    job: &Path,
    started: Instant,
) -> Result<Value, Error> {
    let result = result.and_then(|output| write_json(&job.join("result.json"), &output));
    match result {
        Ok(digest) => {
            receipt["outcome"] = json!("COMPLETED");
            receipt["output"] = json!({"file":"result.json","sha256":digest});
        }
        Err(error) => {
            crate::_4_simulate::control::preserve(&mut receipt, &error);
            let outcome = if error.code == "NEEDS_INPUT" {
                "NEEDS_INPUT"
            } else if error.code.starts_with("UNSUPPORTED_")
                || error.code == "SOURCE_UNAVAILABLE"
                || error.code == "CAPABILITY_UNAVAILABLE"
            {
                "UNSUPPORTED"
            } else if matches!(
                error.code.as_str(),
                "MISMATCH" | "TRACE_PARITY_MISMATCH" | "DEBUG_PARITY_MISMATCH"
            ) {
                "MISMATCH"
            } else if matches!(error.code.as_str(), "WORKER_CANCELLED" | "CANCELLED") {
                "CANCELLED"
            } else if matches!(error.code.as_str(), "WORKER_TIMEOUT" | "SOURCE_DEADLINE") {
                "TIMEOUT"
            } else {
                "ERROR"
            };
            receipt["outcome"] = json!(outcome);
            receipt["incomplete"] = super::super::_1_incomplete::describe(&error, &receipt);
            receipt["error"] = serde_json::to_value(error)
                .map_err(|e| Error::new("OUTPUT_JSON", e.to_string()))?;
        }
    }
    receipt["elapsedMs"] = json!(started.elapsed().as_millis());
    write_json(&job.join("receipt.json"), &receipt)?;
    // Location is a convenience field, not part of the immutable semantic receipt.
    receipt["receiptPath"] = json!(job.join("receipt.json"));
    Ok(receipt)
}
