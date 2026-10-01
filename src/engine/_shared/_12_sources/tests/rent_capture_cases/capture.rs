use super::*;

pub(super) fn capture(
    directory: &Path,
    queries: &[Value],
    query_sha256: &Digest,
    make_source: impl FnOnce() -> Result<AlchemySource>,
) -> Result<Value> {
    validate_queries(queries)?;
    // create_dir is exclusive: an existing directory/symlink fails before the
    // factory can read an environment credential or issue any network request.
    create_private(directory)?;
    create_private(&directory.join("records"))?;
    write_new(
        &directory.join("started.json"),
        &json!({"schema":"svm-replay-m17-exact-rent-started/v1","queryFileSha256":query_sha256,
        "queries":queries,"limits":{"maxRequests":17,"maxAccountReads":16,"maxResponseBytes":64*1024,"maxDownloadBytes":2*1024*1024,"deadlineMs":120000},"retries":0}),
    )?;
    let source = make_source();
    let mut entries = Vec::new();
    let mut accepted = 0;
    let mut failure = None;
    let mut failed_query = Value::Null;
    if let Ok(source) = &source {
        for q in queries {
            let attempt = (|| {
                let record = source.inspect(q)?.ok_or_else(|| {
                    SourceError::new("SOURCE_UNAVAILABLE", "exact Rent source unavailable")
                })?;
                let bytes = serde_json::to_vec(&record).map_err(|e| invalid(e.to_string()))?;
                let digest = Digest::of(&bytes);
                let file = format!("records/{}.json", digest.as_str());
                write_new(&directory.join(&file), &record)?;
                entries.push(json!({"query":q,"file":file,"sha256":digest}));
                // Keep the source's normalized role intact for raw-provenance
                // verification; classify a separate copy for Rent validation.
                let mut roles = svm_replay_engine::shared::history::Roles::default();
                roles.sysvars.insert(RENT.into());
                let slot = checked_query(q)?;
                let image = svm_replay_engine::shared::history::classify_account(
                    record["value"].clone(),
                    RENT,
                    slot,
                    &roles,
                )
                .map_err(|e| SourceError::new("SOURCE_UNAVAILABLE", e.to_string()))?;
                svm_replay_engine::shared::bank::sysvars::bind_exact_generic_sysvar(&image, slot)
                    .map_err(|e| SourceError::new("SOURCE_INTEGRITY", e.to_string()))?;
                Ok(())
            })();
            match attempt {
                Ok(()) => accepted += 1,
                Err(error) => {
                    failure = Some(error);
                    failed_query = q.clone();
                    break;
                }
            }
        }
    } else if let Err(error) = &source {
        failure = Some(error.clone());
    }
    let counters = match &source {
        Ok(source) => source.counters()?,
        Err(_) => SourceCounters::default(),
    };
    let identity = json!({"id":"m17-captured-exact-target-rent","version":"v1","kind":"captured-history","genesisHash":GENESIS,
        "coverage":{"firstSlot":queries[0]["slot"],"lastSlot":queries[15]["slot"],"completeness":"partial"},"capabilities":["account"]});
    let manifest = json!({"schema":"m11-captured-source/v1","identity":identity,"entries":entries});
    let manifest_sha256 = write_new(&directory.join("manifest.json"), &manifest)?;
    let receipt = json!({"schema":"svm-replay-m17-exact-rent-capture/v1","status":if failure.is_none(){"COMPLETE"}else{"STOPPED_ERROR"},
        "queryFileSha256":query_sha256,"manifestSha256":manifest_sha256,"requestedQueries":queries.len(),"recordsCaptured":entries.len(),"acceptedRentImages":accepted,
        "counters":counters,"error":failure,"failedQuery":failed_query,"retries":0,"rawErrorBodyPersisted":false,"currentStateFallback":false});
    write_new(&directory.join("capture-receipt.json"), &receipt)?;
    Ok(receipt)
}

pub(super) fn env(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| invalid(format!("set {name}")))
}

pub(super) fn read_pinned(path_name: &str, hash_name: &str) -> Result<(Value, Digest)> {
    let path = std::fs::canonicalize(env(path_name)?).map_err(io)?;
    require(
        path.starts_with(std::fs::canonicalize(env("SVM_REPLAY_TEST_CAPTURE_ROOT")?).map_err(io)?),
        "input must be under the explicitly approved capture root",
    )?;
    let pin = Digest::new(env(hash_name)?).map_err(|e| invalid(e.to_string()))?;
    require(
        std::fs::metadata(&path).map_err(io)?.len() <= 32 * 1024 * 1024,
        "input exceeds32MiB",
    )?;
    let bytes = std::fs::read(path).map_err(io)?;
    require(Digest::of(&bytes) == pin, "input digest differs")?;
    Ok((
        svm_replay_protocol::parse_json(&bytes).map_err(|e| invalid(e.to_string()))?,
        pin,
    ))
}

pub(super) fn new_output(name: &str) -> Result<std::path::PathBuf> {
    let path = std::path::PathBuf::from(env(name)?);
    require(
        path.is_absolute() && path.file_name().is_some(),
        "output must be absolute",
    )?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid("output parent missing"))?;
    require(
        std::fs::canonicalize(parent).map_err(io)?
            == std::fs::canonicalize(env("SVM_REPLAY_TEST_CAPTURE_ROOT")?).map_err(io)?,
        "output must be a direct child of approved data root",
    )?;
    match std::fs::symlink_metadata(&path) {
        Ok(_) => return Err(invalid("output already exists; no retries or overwrites")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io(error)),
    }
    Ok(path)
}

pub(super) fn query(slot: u64) -> Value {
    json!({"kind":"account","genesisHash":GENESIS,"slot":slot,"pubkey":RENT,"phase":"end-slot"})
}

pub(super) fn synthetic_summary() -> Value {
    let mut results = (0..30)
        .map(|n| json!({"caseId":format!("success-{n}"),"outcome":"COMPLETED","code":0}))
        .collect::<Vec<_>>();
    for n in 0..17 {
        results.push(json!({"caseId":format!("missing-{n}"),"outcome":"UNSUPPORTED","code":1,
            "error":{"code":"SOURCE_UNAVAILABLE","details":{"discovery":{"schema":"svm-sysvar-discovery/v1","status":"NEEDS_INPUT","pubkey":RENT},
            "sourceError":{"code":"SOURCE_UNAVAILABLE","details":{"query":query(100+n%16)}}}}}));
    }
    json!({"schema":"svm-replay-m17-guarded-prepared-parity/v1","status":"INCOMPLETE","count":47,"completed":30,"networkRequests":0,"reconstructedFromSources":false,"results":results})
}

pub(super) struct SyntheticTransport {
    pub(super) calls: Mutex<Vec<Value>>,
    pub(super) failure: Option<u16>,
}

impl RpcTransport for SyntheticTransport {
    fn post(
        &self,
        raw: &[u8],
        timeout: Duration,
        max_response_bytes: u64,
    ) -> std::result::Result<RpcHttpResponse, TransportFailure> {
        assert!(timeout <= Duration::from_secs(120) && !timeout.is_zero());
        assert_eq!(max_response_bytes, 64 * 1024);
        let request = svm_replay_protocol::parse_json(raw).unwrap();
        self.calls.lock().unwrap().push(request.clone());
        let body = if request["method"] == "getGenesisHash" {
            serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"result":GENESIS})).unwrap()
        } else {
            assert_eq!(request[0]["method"], "getAccountInfo");
            assert_eq!(request[0]["params"][0], RENT);
            if let Some(status) = self.failure {
                return Ok(RpcHttpResponse {
                    status,
                    body: if status == 200 {
                        serde_json::to_vec(&json!([{"jsonrpc":"2.0","id":0,"error":{"code":-32005,"message":"sensitive-provider-error-not-persisted"}}])).unwrap()
                    } else {
                        b"sensitive-provider-error-not-persisted".to_vec()
                    },
                });
            }
            serde_json::to_vec(&json!([{"jsonrpc":"2.0","id":0,"result":{"context":{"slot":request[0]["params"][1]["slot"]},
                "value":{"lamports":9007199254740993u64,"rentEpoch":u64::MAX,"owner":"Sysvar1111111111111111111111111111111111111",
                "executable":false,"data":["MBsAAAAAAAAAAAAAAADwPzI=","base64"]}}}])).unwrap()
        };
        Ok(RpcHttpResponse { status: 200, body })
    }
}
