use super::*;

pub(in super::super) const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

pub(in super::super) const MAX_PAYLOAD_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportCacheIdentity {
    pub input_sha256: Digest,
    pub worker: WorkerDescriptor,
    pub request: CaptureRequest,
    pub producer_bounds: ProducerBounds,
    /// Digest of the caller-validated immutable session or gated execution context.
    pub context_sha256: Digest,
    /// Keep the producer's artifact identity, including when importing a pinned legacy export.
    pub artifact_identity_sha256: Digest,
}

impl ExportCacheIdentity {
    pub fn key(&self) -> Result<Digest, Error> {
        let capture = capture_identity(
            &self.input_sha256,
            &self.worker,
            &self.request,
            &self.producer_bounds,
        )?;
        Ok(Digest::of(canonical_json(
            &json!({"schema":"svm-m17-trace-export-key/v1",
            "captureIdentitySha256":capture,"contextSha256":self.context_sha256,
            "artifactIdentitySha256":self.artifact_identity_sha256}),
        )))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in super::super) struct PayloadRef {
    pub(in super::super) key: Digest,
    pub(in super::super) sha256: Digest,
    pub(in super::super) bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in super::super) struct Manifest {
    pub(in super::super) schema: String,
    pub(in super::super) identity: ExportCacheIdentity,
    pub(in super::super) identity_sha256: Digest,
    pub(in super::super) origin_run_sha256: Digest,
    pub(in super::super) artifact: Value,
    pub(in super::super) payload_ref: Option<PayloadRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCacheReceipt {
    pub schema: &'static str,
    pub identity_sha256: Digest,
    pub manifest_sha256: Digest,
    pub origin_run_sha256: Digest,
    pub payload_sha256: Option<Digest>,
    pub payload_bytes: u64,
    pub reused: bool,
    /// Always false: cache storage/retrieval does not execute a worker.
    pub new_execution: bool,
}

pub struct CachedExport {
    pub export: CaptureExport,
    pub receipt: ExportCacheReceipt,
}

pub(in super::super) fn corruption(message: impl Into<String>) -> Error {
    Error::new("CAPTURE_CACHE_CORRUPTION", message)
}

pub(in super::super) fn store_error(error: svm_replay_store::StoreError) -> Error {
    Error::new(format!("CAPTURE_CACHE_STORE_{}", error.code), error.message)
        .with_details(json!({"retryable":error.retryable,"storeDetails":error.details}))
}

pub(in super::super) fn payload_key(sha256: &Digest) -> Digest {
    Digest::of(canonical_json(
        &json!({"schema":"svm-m17-trace-export-payload/v1","sha256":sha256}),
    ))
}

pub(in super::super) fn get(store: &mut Store, key: &Digest) -> Result<Option<Vec<u8>>, Error> {
    let response = store
        .execute(json!({"version":1,"op":"get","namespace":"raw","key":key}))
        .map_err(store_error)?;
    if response["status"] == "MISS" {
        return Ok(None);
    }
    if response["status"] != "HIT" {
        return Err(corruption("unknown Store get status"));
    }
    let encoded = response["dataBase64"]
        .as_str()
        .ok_or_else(|| corruption("Store hit has no bytes"))?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| corruption(error.to_string()))?;
    if STANDARD.encode(&bytes) != encoded
        || response["sha256"] != json!(Digest::of(&bytes))
        || response["sizeBytes"] != json!(bytes.len())
    {
        return Err(corruption("Store hit byte identity differs"));
    }
    Ok(Some(bytes))
}

pub(in super::super) fn put(store: &mut Store, key: &Digest, bytes: &[u8]) -> Result<(), Error> {
    let response = store
        .execute(json!({"version":1,"op":"put","namespace":"raw","key":key,
        "dataBase64":STANDARD.encode(bytes)}))
        .map_err(store_error)?;
    if response["status"] != "STORED"
        || response["sha256"] != json!(Digest::of(bytes))
        || response["sizeBytes"] != json!(bytes.len())
    {
        return Err(corruption("Store put acknowledgement differs"));
    }
    Ok(())
}

pub(in super::super) fn receipt(
    manifest: &Manifest,
    bytes: &[u8],
    reused: bool,
) -> ExportCacheReceipt {
    ExportCacheReceipt {
        schema: "svm-m17-trace-export-cache-receipt/v1",
        identity_sha256: manifest.identity_sha256.clone(),
        manifest_sha256: Digest::of(bytes),
        origin_run_sha256: manifest.origin_run_sha256.clone(),
        payload_sha256: manifest
            .payload_ref
            .as_ref()
            .map(|payload| payload.sha256.clone()),
        payload_bytes: match &manifest.payload_ref {
            Some(payload) => payload.bytes,
            None => 0,
        },
        reused,
        new_execution: false,
    }
}

pub(in super::super) fn validate_export(
    identity: &ExportCacheIdentity,
    export: &CaptureExport,
) -> Result<(), Error> {
    let payload = if export.artifact["payload"].is_null() {
        if !export.payload.is_empty() {
            return Err(Error::new(
                "CAPTURE_INTEGRITY",
                "absent artifact payload carries bytes",
            ));
        }
        None
    } else {
        Some(export.payload.as_slice())
    };
    validate_artifact(
        &export.artifact,
        &identity.request,
        &identity.artifact_identity_sha256,
        payload,
    )
}
