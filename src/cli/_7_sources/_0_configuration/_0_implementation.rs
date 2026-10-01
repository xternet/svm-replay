use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in super::super) struct LiveConfig {
    pub(in super::super) id: String,
    pub(in super::super) version: String,
    pub(in super::super) genesis_hash: String,
    pub(in super::super) first_slot: u64,
    pub(in super::super) last_slot: u64,
    pub(in super::super) max_requests: u64,
    pub(in super::super) max_account_reads: u64,
    pub(in super::super) max_download_bytes: u64,
    pub(in super::super) max_response_bytes: u64,
    pub(in super::super) durable_budget_id: Option<String>,
}

pub fn configured(
    captured: Option<(PathBuf, Digest)>,
    alchemy: Option<PathBuf>,
    data_dir: &Path,
    cache: CacheMode,
    timeout_ms: u64,
) -> Result<Option<CompositeSource>, Error> {
    let mut inputs: Vec<(Arc<dyn HistoricalSource>, String)> = Vec::new();
    if let Some((path, pin)) = captured {
        let source = CapturedSource::open(&path, &pin).map_err(source_error)?;
        inputs.push((Arc::new(source), format!("captured:{}", pin.as_str())));
    }
    if let Some(path) = alchemy {
        inputs.push((
            Arc::new(alchemy_source(&path, data_dir, timeout_ms)?),
            "alchemy".into(),
        ));
    }
    finish_sources(inputs, data_dir, cache)
}

pub fn alchemy_source(
    path: &Path,
    data_dir: &Path,
    timeout_ms: u64,
) -> Result<AlchemySource, Error> {
    let bytes = read_bounded_file(&path, 1024 * 1024)
        .map_err(|e| Error::new("SOURCE_CONFIGURATION", e.to_string()))?;
    let file: LiveConfig = serde_json::from_value(parse_json(&bytes)?)
        .map_err(|e| Error::new("SOURCE_CONFIGURATION", e.to_string()))?;
    let limits = SourceLimits {
        max_requests: file.max_requests,
        max_account_reads: file.max_account_reads,
        max_download_bytes: file.max_download_bytes,
        max_response_bytes: file.max_response_bytes,
        deadline: Duration::from_millis(timeout_ms),
    };
    let budget = file
        .durable_budget_id
        .map(|id| {
            let cache_dir = svm_replay_engine::shared::layout::cache_dir(data_dir)?;
            svm_replay_engine::_5_finalize::ensure_private_directory(data_dir)?;
            let store = svm_replay_store::Store::open(&cache_dir)
                .map_err(|e| Error::new(format!("STORE_{}", e.code), e.message))?;
            DurableRpcBudget::open(store, id, &limits)
                .map(Arc::new)
                .map_err(source_error)
        })
        .transpose()?;
    let mut source = AlchemySource::from_environment(
        AlchemyConfig {
            id: file.id,
            version: file.version,
            expected_genesis_hash: file.genesis_hash,
            first_slot: file.first_slot,
            last_slot: file.last_slot,
        },
        limits,
    )
    .map_err(source_error)?;
    if let Some(budget) = budget {
        source = source.with_budget(budget).map_err(source_error)?;
    }
    Ok(source)
}

pub(crate) fn finish_sources(
    inputs: Vec<(Arc<dyn HistoricalSource>, String)>,
    data_dir: &Path,
    cache: CacheMode,
) -> Result<Option<CompositeSource>, Error> {
    if inputs.is_empty() {
        return Ok(None);
    }
    let mut sources: Vec<Box<dyn HistoricalSource>> = Vec::new();
    if cache == CacheMode::Off {
        for (source, _) in inputs {
            sources.push(Box::new(SharedSource(source)));
        }
    } else {
        let cache_dir = svm_replay_engine::shared::layout::cache_dir(data_dir)?;
        svm_replay_engine::_5_finalize::ensure_private_directory(data_dir)?;
        let store = svm_replay_store::Store::open(&cache_dir)
            .map_err(|e| Error::new(format!("STORE_{}", e.code), e.message))?;
        let store = Arc::new(Mutex::new(store));
        for (source, namespace_id) in inputs {
            let expected_source_identity_sha256 =
                source_identity_sha256(&source.identity()).map_err(source_error)?;
            sources.push(Box::new(
                CachedSource::new(
                    source,
                    store.clone(),
                    CacheScope {
                        namespace_id,
                        expected_source_identity_sha256,
                        pin_owner: None,
                    },
                )
                .map_err(source_error)?,
            ));
        }
    }
    CompositeSource::new(sources)
        .map(Some)
        .map_err(source_error)
}

pub(in super::super) struct SharedSource(pub(in super::super) Arc<dyn HistoricalSource>);

impl HistoricalSource for SharedSource {
    fn identity(&self) -> serde_json::Value {
        self.0.identity()
    }
    fn diagnostics(&self) -> svm_replay_engine::shared::sources::Result<serde_json::Value> {
        self.0.diagnostics()
    }
    fn inspect(
        &self,
        query: &serde_json::Value,
    ) -> svm_replay_engine::shared::sources::Result<Option<serde_json::Value>> {
        self.0.inspect(query)
    }
}
