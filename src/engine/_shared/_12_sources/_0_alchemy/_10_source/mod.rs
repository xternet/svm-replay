use super::*;

impl HistoricalSource for AlchemySource {
    fn discover_block(&self, slot: u64) -> Result<Option<Value>> {
        AlchemySource::discover_block(self, slot).map(Some)
    }
    fn identity(&self) -> Value {
        self.identity.clone()
    }
    fn diagnostics(&self) -> Result<Value> {
        let counters = self.counters()?;
        let durable = self.budget.as_ref().map(|budget| match budget.snapshot() {
            Ok(value) => value,
            Err(error) => json!({"status":"ERROR","error":error}),
        });
        Ok(
            json!({"status":"REPORTED","transport":counters,"durableBudget":durable,
            "downloadAccounting":"fully-observed-response-bodies; failed partial bodies are unknown",
            "chargedAccounting":if self.budget.is_some(){"full-response-reservation/no-refund"}else{"observed bytes plus conservative failed-transport allowance"}}),
        )
    }
    fn inspect(&self, query: &Value) -> Result<Option<Value>> {
        let result = (|| {
            let mut state = self
                .state
                .lock()
                .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "source state poisoned"))?;
            state.inspected = true;
            query_key(query)?;
            if !covers(&self.identity, query)? {
                return Ok(None);
            }
            if state.genesis_raw.is_none() {
                let raw = self.post(
                    &mut state,
                    &json!({"jsonrpc":"2.0","id":1,"method":"getGenesisHash","params":[]}),
                    false,
                )?;
                checked_genesis(&raw, &self.identity["genesisHash"])?;
                state.genesis_raw = Some(raw);
            }
            let mut request = request_for_query(query)?;
            let mut raw = self.post(&mut state, &request, query["kind"] == "account")?;
            if query["kind"] == "transaction" && _11_lookup::missing(&raw, true)? {
                request = _11_lookup::block_request(query);
                raw = self.post(&mut state, &request, false)?;
            }
            let genesis = state.genesis_raw.as_ref().ok_or_else(|| {
                SourceError::new("SOURCE_INTEGRITY", "genesis verification state missing")
            })?;
            record_from_response(query, &request, &raw, genesis).map(Some)
        })();
        if result.is_err() {
            let mut state = self.state.lock().map_err(|_| {
                SourceError::new(
                    "SOURCE_INTEGRITY",
                    "source state poisoned while recording failure",
                )
            })?;
            state.counters.failed_inspections += 1;
        }
        result
    }
}
