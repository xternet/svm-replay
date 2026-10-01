use super::*;

impl AlchemySource {
    pub fn with_discovery_cache(mut self, store: Store) -> Self {
        self.discovery_cache = Some(Mutex::new(store));
        self
    }
    pub(in super::super) fn cached_discovery(
        &self,
        query: Value,
        fetch: impl FnOnce() -> Result<Value>,
        validate: impl Fn(&Value) -> Result<()>,
    ) -> Result<Value> {
        self.check_cancelled()?;
        self.remaining()?;
        let Some(store) = &self.discovery_cache else {
            let value = fetch()?;
            validate(&value)?;
            return Ok(value);
        };
        let mut store = store
            .lock()
            .map_err(|_| SourceError::new("SOURCE_CACHE_ERROR", "discovery cache lock poisoned"))?;
        let identity =
            json!({"schema":"svm-finalized-discovery/v1","source":self.identity,"query":query});
        let key = format!(
            "discovery/{}",
            Digest::of(serde_json::to_vec(&identity).map_err(|_| SourceError::new(
                "SOURCE_INTEGRITY",
                "discovery identity serialization"
            ))?)
            .as_str()
        );
        let execute = |store: &mut Store, request| {
            store.execute(request).map_err(|e| {
                SourceError::new("SOURCE_CACHE_ERROR", format!("discovery store: {}", e.code))
            })
        };
        let cached = execute(
            &mut store,
            json!({"version":1,"op":"get","namespace":"raw","key":key}),
        )?;
        if cached["status"] == "HIT" {
            let bytes = canonical_base64(&cached["dataBase64"], "discovery cache bytes")?;
            require(
                cached["sha256"] == json!(Digest::of(&bytes)),
                "SOURCE_INTEGRITY",
                "discovery hash differs",
            )?;
            let entry = parse_source_json(&bytes)?;
            object(
                &entry,
                &["identity", "value"],
                "SOURCE_INTEGRITY",
                "discovery entry",
            )?;
            require(
                entry["identity"] == identity,
                "SOURCE_INTEGRITY",
                "discovery context differs",
            )?;
            validate(&entry["value"])?;
            return Ok(entry["value"].clone());
        }
        require(
            cached["status"] == "MISS",
            "SOURCE_CACHE_ERROR",
            "unexpected discovery cache status",
        )?;
        let value = fetch()?;
        validate(&value)?;
        let bytes = serde_json::to_vec(&json!({"identity":identity,"value":value}))
            .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "discovery serialization"))?;
        execute(
            &mut store,
            json!({"version":1,"op":"put","namespace":"raw","key":key,"dataBase64":STANDARD.encode(bytes)}),
        )?;
        Ok(value)
    }
    pub fn discover_block(&self, target_slot: u64) -> Result<Value> {
        self.cached_discovery(
            json!({"kind":"block","slot":target_slot,"rewards":true}),
            || self.discover_block_uncached(target_slot),
            |record| {
                let query = &record["query"];
                require(
                    query["kind"] == "block"
                        && query["slot"] == target_slot
                        && query["genesisHash"] == self.identity["genesisHash"],
                    "SOURCE_CONTEXT_MISMATCH",
                    "cached discovery block context differs",
                )?;
                validate_record(record, query)
            },
        )
    }
}
