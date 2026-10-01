use super::*;
pub(super) fn admit_cached_result(
    record: &Value,
    keys: &identity::CacheIdentity,
    implementation_sha256: &Digest,
    limits: &WorkerLimits,
) -> Result<u64, Error> {
    if record["schema"] != "svm-m17-cached-result/v1"
        || record["preparedKey"] != json!(keys.prepared_key)
        || record["resultKey"] != json!(keys.result_key)
        || record["implementationSha256"] != json!(implementation_sha256)
    {
        return Err(Error::new(
            "RESULT_IDENTITY",
            "cached result identity differs",
        ));
    }
    Digest::new(
        field(&record, "checkpointSha256")?
            .as_str()
            .ok_or_else(|| Error::new("RESULT_IDENTITY", "checkpoint digest missing"))?
            .to_owned(),
    )?;
    let checkpoint_bytes = uint(field(&record, "checkpointBytes")?, "checkpoint bytes")?;
    if checkpoint_bytes > limits.max_output_bytes as u64 {
        return Err(Error::new(
            "WORKER_OUTPUTLIMIT",
            "cached checkpoint exceeds current file limit",
        ));
    }
    Ok(checkpoint_bytes)
}
