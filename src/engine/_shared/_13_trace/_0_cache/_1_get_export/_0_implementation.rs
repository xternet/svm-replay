use super::*;

/// Reopen an exact immutable export. A missing manifest is a miss; missing or
/// corrupt referenced bytes are errors, never regeneration or partial success.
pub fn get_export(
    store: &mut Store,
    identity: &ExportCacheIdentity,
) -> Result<Option<CachedExport>, Error> {
    let key = identity.key()?;
    let Some(bytes) = get(store, &key)? else {
        return Ok(None);
    };
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(corruption("export manifest exceeds 1 MiB"));
    }
    let value = parse_json(&bytes).map_err(|error| corruption(error.message))?;
    super::super::super::contract::shape(
        &value,
        &[
            "schema",
            "identity",
            "identitySha256",
            "originRunSha256",
            "artifact",
            "payloadRef",
        ],
    )
    .map_err(|error| corruption(error.message))?;
    let manifest: Manifest =
        serde_json::from_value(value).map_err(|error| corruption(error.to_string()))?;
    if manifest.schema != "svm-m17-trace-export/v1"
        || manifest.identity_sha256 != key
        || manifest
            .identity
            .key()
            .map_err(|error| corruption(error.message))?
            != key
    {
        return Err(corruption(
            "export schema/context/policy/worker identity differs",
        ));
    }
    let payload = match &manifest.payload_ref {
        Some(reference) => {
            if reference.bytes > MAX_PAYLOAD_BYTES
                || reference.bytes > identity.request.limits.max_bytes
                || reference.key != payload_key(&reference.sha256)
                || manifest.artifact["payload"]["sha256"] != json!(reference.sha256)
                || manifest.artifact["payload"]["bytes"] != json!(reference.bytes)
            {
                return Err(corruption("export payload reference/bound differs"));
            }
            let payload = get(store, &reference.key)?
                .ok_or_else(|| corruption("export payload is missing"))?;
            if payload.len() as u64 != reference.bytes || Digest::of(&payload) != reference.sha256 {
                return Err(corruption("export payload bytes/digest differ"));
            }
            payload
        }
        None => {
            if !manifest.artifact["payload"].is_null() {
                return Err(corruption("export payload reference is missing"));
            }
            Vec::new()
        }
    };
    let proof = receipt(&manifest, &bytes, true);
    let export = CaptureExport {
        artifact: manifest.artifact,
        payload,
    };
    validate_export(identity, &export).map_err(|error| corruption(error.message))?;
    Ok(Some(CachedExport {
        export,
        receipt: proof,
    }))
}

/// Publish payload first, small manifest last. Identical observations preserve
/// the first origin; conflicting observations are never overwritten.
pub fn put_export(
    store: &mut Store,
    identity: &ExportCacheIdentity,
    origin_run_sha256: &Digest,
    export: &CaptureExport,
) -> Result<ExportCacheReceipt, Error> {
    let key = identity.key()?;
    validate_export(identity, export)?;
    if let Some(existing) = get_export(store, identity)? {
        if existing.export.artifact != export.artifact || existing.export.payload != export.payload
        {
            return Err(Error::new(
                "CAPTURE_CACHE_CONFLICT",
                "different observations already exist for exact export identity",
            ));
        }
        return Ok(existing.receipt);
    }
    let payload_ref = if export.artifact["payload"].is_null() {
        None
    } else {
        let sha256 = Digest::of(&export.payload);
        Some(PayloadRef {
            key: payload_key(&sha256),
            sha256,
            bytes: export.payload.len() as u64,
        })
    };
    let manifest = Manifest {
        schema: "svm-m17-trace-export/v1".into(),
        identity: identity.clone(),
        identity_sha256: key.clone(),
        origin_run_sha256: origin_run_sha256.clone(),
        artifact: export.artifact.clone(),
        payload_ref,
    };
    let bytes = canonical_json(
        &serde_json::to_value(&manifest).map_err(|error| corruption(error.to_string()))?,
    )
    .into_bytes();
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(Error::new("CAPTURE_LIMIT", "export manifest exceeds 1 MiB"));
    }
    if let Some(reference) = &manifest.payload_ref {
        put(store, &reference.key, &export.payload)?;
    }
    put(store, &key, &bytes)?;
    Ok(receipt(&manifest, &bytes, false))
}
