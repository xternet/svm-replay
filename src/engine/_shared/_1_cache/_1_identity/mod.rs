use crate::shared::diff::canonical_json;
use serde_json::{json, Value};
use svm_replay_protocol::{worker::WorkerDescriptor, Digest, Error, PreparedRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheIdentity {
    pub prepared_key: Digest,
    pub result_key: Digest,
}

/// Complete prepared input identity. Collection order is retained because the
/// native checkpoint's own fixture hash also retains array order.
pub fn derive(
    request: &PreparedRequest,
    worker: &WorkerDescriptor,
    catalog: &Digest,
    implementation: &Digest,
) -> Result<CacheIdentity, Error> {
    let target = request
        .fixture
        .get("target")
        .ok_or_else(|| Error::new("CACHE_IDENTITY", "missing fixture target"))?;
    let replacement = target
        .get("replacementTransactionBase64")
        .ok_or_else(|| Error::new("CACHE_IDENTITY", "missing replacement field"))?;
    let account_override = request
        .fixture
        .get("accountOverride")
        .ok_or_else(|| Error::new("CACHE_IDENTITY", "missing account override field"))?;
    let mut canonical = request.fixture.clone();
    canonical["target"]["replacementTransactionBase64"] = Value::Null;
    canonical["accountOverride"] = Value::Null;
    canonical
        .as_object_mut()
        .ok_or_else(|| Error::new("CACHE_IDENTITY", "fixture must be an object"))?
        .remove("requestedAccountOverrides");
    let prepared_key = Digest::of(canonical_json(
        &json!({"schema":"svm-m17-prepared-key/v1", "fixture":canonical,
        "candidate":request.candidate,"family":request.family,"blockSha256":request.block_sha256,
        "sourceEvidenceHashes":request.source_evidence_hashes,"metadataPolicy":request.metadata_policy,
        "worker":worker,"catalogSha256":catalog,"implementationSha256":implementation}),
    ));
    let mut variant = json!({"schema":"svm-m17-result-key/v1","preparedKey":prepared_key,
        "replacementTransactionBase64":replacement,"accountOverride":account_override,"capture":"existing-runtime-trace/v1"});
    if let Some(overrides) = request.fixture.get("requestedAccountOverrides") {
        variant["requestedAccountOverrides"] = overrides.clone();
    }
    Ok(CacheIdentity {
        prepared_key,
        result_key: Digest::of(canonical_json(&variant)),
    })
}

/// Match the historical native worker codec, not a different cache-key codec.
pub fn checkpoint_fixture_hash(fixture: &Value) -> Result<Digest, Error> {
    let mut value = fixture.clone();
    let object = value
        .as_object_mut()
        .ok_or_else(|| Error::new("CACHE_IDENTITY", "fixture must be an object"))?;
    object.remove("accountOverride");
    object.remove("requestedAccountOverrides");
    object
        .get_mut("target")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| Error::new("CACHE_IDENTITY", "missing target"))?
        .remove("replacementTransactionBase64");
    Ok(Digest::of(serde_json::to_vec(&value).map_err(|error| {
        Error::new("CACHE_IDENTITY", error.to_string())
    })?))
}
