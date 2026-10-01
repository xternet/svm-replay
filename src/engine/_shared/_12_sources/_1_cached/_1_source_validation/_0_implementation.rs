use super::*;

impl HistoricalSource for CachedSource {
    fn discover_block(&self, slot: u64) -> Result<Option<Value>> {
        self.check_identity()?;
        // Discovery has its own provider cache: no invented pre-acquisition pin
        // and no second network fetch through inspect().
        let record = self.inner.discover_block(slot)?;
        self.check_identity()?;
        if let Some(record) = &record {
            validate_record(record, &record["query"])?;
            require(
                record["query"]["kind"] == "block"
                    && record["query"]["slot"] == slot
                    && record["query"]["genesisHash"] == self.identity["genesisHash"],
                "SOURCE_CONTEXT_MISMATCH",
                "discovered block boundary differs",
            )?;
        }
        Ok(record)
    }
    fn identity(&self) -> Value {
        self.inner.identity()
    }
    fn diagnostics(&self) -> Result<Value> {
        self.check_identity()?;
        let c = &self.counts;
        let underlying = match self.inner.diagnostics() {
            Ok(value) => value,
            Err(error) => json!({"status":"ERROR","error":error}),
        };
        Ok(
            json!({"status":"REPORTED","cache":{"namespace":self.scope.namespace_id,
            "inspectCalls":c.calls.load(Ordering::Relaxed),"hits":c.hits.load(Ordering::Relaxed),
            "misses":c.misses.load(Ordering::Relaxed),"fills":c.fills.load(Ordering::Relaxed),
            "failedInspections":c.errors.load(Ordering::Relaxed)},"underlying":underlying}),
        )
    }
    fn inspect(&self, query: &Value) -> Result<Option<Value>> {
        self.counts.calls.fetch_add(1, Ordering::Relaxed);
        let result = (|| {
            self.check_identity()?;
            query_key(query)?;
            if !covers(&self.identity, query)? {
                return Ok(None);
            }
            let key = raw_cache_key(&self.scope, &self.identity, query)?;
            // Sharing this handle coalesces fills. Independent processes instead rely
            // on Store's atomic immutable-key conflict detection; no global lock claim.
            let mut store = self
                .store
                .lock()
                .map_err(|_| SourceError::new("SOURCE_CACHE_ERROR", "raw cache mutex poisoned"))?;
            let cached = execute(
                &mut store,
                json!({"version":1,"op":"get","namespace":"raw","key":key}),
            )?;
            if cached["status"] == "HIT" {
                self.counts.hits.fetch_add(1, Ordering::Relaxed);
                let bytes =
                    canonical_base64(&cached["dataBase64"], "cached raw observation bytes")?;
                require(
                    cached["sha256"] == json!(Digest::of(&bytes)),
                    "SOURCE_INTEGRITY",
                    "cached raw observation digest mismatch",
                )?;
                let entry = parse_source_json(&bytes)?;
                object(
                    &entry,
                    &["schema", "namespace", "source", "query", "record"],
                    "SOURCE_INTEGRITY",
                    "raw cache entry",
                )?;
                require(
                    entry["schema"] == "m17-raw-source-cache/v1"
                        && entry["namespace"] == self.scope.namespace_id
                        && entry["source"] == self.identity
                        && entry["query"] == *query,
                    "SOURCE_INTEGRITY",
                    "raw cache context mismatch",
                )?;
                validate_record(&entry["record"], query)?;
                self.check_identity()?;
                self.pin(&mut store, &key)?;
                return Ok(Some(entry["record"].clone()));
            }
            require(
                cached["status"] == "MISS",
                "SOURCE_CACHE_ERROR",
                "unexpected raw cache status",
            )?;
            self.counts.misses.fetch_add(1, Ordering::Relaxed);
            let record = self.inner.inspect(query)?;
            self.check_identity()?;
            if let Some(record) = record {
                validate_record(&record, query)?;
                let entry = json!({"schema":"m17-raw-source-cache/v1","namespace":self.scope.namespace_id,"source":self.identity,"query":query,"record":record});
                let bytes = serde_json::to_vec(&entry)
                    .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "raw cache serialization"))?;
                execute(
                    &mut store,
                    json!({"version":1,"op":"put","namespace":"raw","key":key,"dataBase64":STANDARD.encode(bytes)}),
                )?;
                self.counts.fills.fetch_add(1, Ordering::Relaxed);
                self.pin(&mut store, &key)?;
                Ok(Some(record))
            } else {
                Ok(None)
            }
        })();
        if result.is_err() {
            self.counts.errors.fetch_add(1, Ordering::Relaxed);
        }
        result
    }
}
