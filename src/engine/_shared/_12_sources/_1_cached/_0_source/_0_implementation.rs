use super::*;

#[derive(Debug, Clone)]
pub struct CacheScope {
    pub namespace_id: String,
    pub expected_source_identity_sha256: Digest,
    pub pin_owner: Option<String>,
}

pub fn source_identity_sha256(identity: &Value) -> Result<Digest> {
    validate_identity(identity)?;
    Ok(Digest::of(serde_json::to_vec(identity).map_err(|_| {
        SourceError::new("INVALID_SOURCE", "source identity serialization")
    })?))
}

pub fn raw_cache_key(scope: &CacheScope, identity: &Value, query: &Value) -> Result<String> {
    public_id(
        &json!(scope.namespace_id),
        "INVALID_SOURCE",
        "cache namespace identity",
    )?;
    require(
        source_identity_sha256(identity)? == scope.expected_source_identity_sha256,
        "INVALID_SOURCE",
        "cache source identity pin mismatch",
    )?;
    query_key(query)?;
    let bound = json!({"schema":"m17-raw-source-cache-key/v1","namespace":scope.namespace_id,"source":identity,"query":query});
    Ok(format!(
        "m17-raw-source/v1/{}",
        Digest::of(
            serde_json::to_vec(&bound)
                .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "cache key serialization"))?
        )
        .as_str()
    ))
}

pub struct CachedSource {
    pub(in super::super) inner: Arc<dyn HistoricalSource>,
    pub(in super::super) identity: Value,
    pub(in super::super) store: Arc<Mutex<Store>>,
    pub(in super::super) scope: CacheScope,
    pub(in super::super) counts: CacheCounters,
}

#[derive(Default)]
pub(in super::super) struct CacheCounters {
    pub(in super::super) calls: AtomicU64,
    pub(in super::super) hits: AtomicU64,
    pub(in super::super) misses: AtomicU64,
    pub(in super::super) fills: AtomicU64,
    pub(in super::super) errors: AtomicU64,
}

impl CachedSource {
    pub fn new(
        inner: Arc<dyn HistoricalSource>,
        store: Arc<Mutex<Store>>,
        scope: CacheScope,
    ) -> Result<Self> {
        let identity = inner.identity();
        validate_identity(&identity)?;
        public_id(
            &json!(scope.namespace_id),
            "INVALID_SOURCE",
            "cache namespace identity",
        )?;
        require(
            source_identity_sha256(&identity)? == scope.expected_source_identity_sha256,
            "INVALID_SOURCE",
            "cache source identity pin mismatch",
        )?;
        if let Some(owner) = &scope.pin_owner {
            public_id(&json!(owner), "INVALID_SOURCE", "cache pin owner")?;
        }
        Ok(Self {
            inner,
            identity,
            store,
            scope,
            counts: CacheCounters::default(),
        })
    }
    pub(in super::super) fn check_identity(&self) -> Result<()> {
        require(
            self.inner.identity() == self.identity,
            "SOURCE_INTEGRITY",
            "cached provider identity mutated",
        )
    }
    pub(in super::super) fn pin(&self, store: &mut Store, key: &str) -> Result<()> {
        if let Some(owner) = &self.scope.pin_owner {
            execute(
                store,
                json!({"version":1,"op":"pin","namespace":"raw","key":key,"owner":owner}),
            )?;
        }
        Ok(())
    }
}

pub(in super::super) fn execute(store: &mut Store, request: Value) -> Result<Value> {
    store.execute(request).map_err(|e| {
        SourceError::new(
            if e.code == "CONFLICT" {
                "SOURCE_CONFLICT"
            } else {
                "SOURCE_CACHE_ERROR"
            },
            "raw source cache operation failed",
        )
        .details(json!({"storeCode":e.code}))
    })
}
