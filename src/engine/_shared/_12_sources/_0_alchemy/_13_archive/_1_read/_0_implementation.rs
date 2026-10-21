use super::*;

/// The JSON-RPC result of an archive read; a provider error is the read's failure.
fn result(raw: &[u8]) -> Result<Value> {
    envelope(&parse(raw)?, 1).cloned()
}

impl AlchemySource {
    /// A finalized archive read, pinned with its request and response like every discovery.
    /// The result is returned whole: the caller applies the fork's position to it.
    pub fn archive(&self, query: &Value) -> Result<Value> {
        self.archive_inner(query)
            .map_err(|error| attribute_source_failure(error, &self.identity))
    }
    fn archive_inner(&self, query: &Value) -> Result<Value> {
        let request = request(query)?;
        let read = self.cached_discovery(
            json!({"kind":"archive","query":query}),
            || {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "source state poisoned"))?;
                state.inspected = true;
                if state.genesis_raw.is_none() {
                    let raw = self.post(
                        &mut state,
                        &json!({"jsonrpc":"2.0","id":1,"method":"getGenesisHash","params":[]}),
                        false,
                    )?;
                    checked_genesis(&raw, &self.identity["genesisHash"])?;
                    state.genesis_raw = Some(raw);
                }
                let raw = self.post(&mut state, &request, false)?;
                result(&raw)?;
                let genesis = state.genesis_raw.as_ref().ok_or_else(|| {
                    SourceError::new("SOURCE_INTEGRITY", "archive genesis missing")
                })?;
                Ok(
                    json!({"schema":"svm-archive-read/v1","query":query,"request":request,
                "responseBodyBase64":STANDARD.encode(&raw),"responseSha256":Digest::of(&raw),
                "genesisResponseBodyBase64":STANDARD.encode(genesis)}),
                )
            },
            |value| {
                object(
                    value,
                    &[
                        "schema",
                        "query",
                        "request",
                        "responseBodyBase64",
                        "responseSha256",
                        "genesisResponseBodyBase64",
                    ],
                    "SOURCE_INTEGRITY",
                    "archive read",
                )?;
                require(
                    value["schema"] == "svm-archive-read/v1"
                        && value["query"] == *query
                        && value["request"] == request,
                    "SOURCE_CONTEXT_MISMATCH",
                    "cached archive request differs",
                )?;
                let raw = canonical_base64(&value["responseBodyBase64"], "archive response")?;
                let genesis =
                    canonical_base64(&value["genesisResponseBodyBase64"], "archive genesis")?;
                checked_genesis(&genesis, &self.identity["genesisHash"])?;
                require(
                    value["responseSha256"] == json!(Digest::of(&raw)),
                    "SOURCE_INTEGRITY",
                    "cached archive response differs",
                )?;
                result(&raw).map(|_| ())
            },
        )?;
        result(&canonical_base64(
            &read["responseBodyBase64"],
            "archive response",
        )?)
    }
}
