use super::*;

pub(super) fn transaction_slot(raw: &[u8], signature: &str) -> Result<u64> {
    let value = parse(raw)?;
    let result = envelope(&value, 1)?;
    require(
        result.is_object(),
        "SOURCE_UNAVAILABLE",
        "transaction not found",
    )?;
    let signatures = result
        .pointer("/transaction/signatures")
        .and_then(Value::as_array)
        .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "transaction signatures missing"))?;
    require(
        signatures.first().and_then(Value::as_str) == Some(signature),
        "SOURCE_CONTEXT_MISMATCH",
        "returned transaction signature differs",
    )?;
    slot(&result["slot"], "SOURCE_INTEGRITY", "transaction slot")
}

impl AlchemySource {
    /// Find the target slot; retain raw lookup evidence instead of guessing dates.
    pub fn discover_transaction(&self, signature: &str) -> Result<Value> {
        self.cached_discovery(json!({"kind":"signature","signature":signature}),
            || self.discover_transaction_uncached(signature), |value| {
                let raw=canonical_base64(&value["responseBodyBase64"],"discovery transaction bytes")?;
                let genesis=canonical_base64(&value["genesisResponseBodyBase64"],"discovery genesis bytes")?;
                checked_genesis(&genesis,&self.identity["genesisHash"])?;
                let slot=if value.get("lookupKind").is_some() {
                    _11_lookup::validate_location(value, signature)?
                } else { transaction_slot(&raw,signature)? };
                require(value["signature"]==signature && value["slot"]==slot && value["responseSha256"]==json!(Digest::of(&raw)),
                    "SOURCE_INTEGRITY","cached signature evidence differs")?;
                let mut request = value["request"].clone();
                require(matches!(request["params"][1]["maxSupportedTransactionVersion"].as_u64(), Some(0 | 1)),
                    "SOURCE_INTEGRITY", "unsupported discovery version bound")?;
                request["params"][1]["maxSupportedTransactionVersion"] = json!(1);
                require(request==json!({"jsonrpc":"2.0","id":1,"method":"getTransaction","params":[signature,{"encoding":"json","commitment":"finalized","maxSupportedTransactionVersion":1}]}),
                    "SOURCE_INTEGRITY","discovery request differs")
            })
    }
    pub(super) fn discover_transaction_uncached(&self, signature: &str) -> Result<Value> {
        address(
            &json!(signature),
            64,
            "INVALID_QUERY",
            "transaction signature",
        )?;
        let result = (|| {
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
            let request = json!({"jsonrpc":"2.0","id":1,"method":"getTransaction","params":[signature,{"encoding":"json","commitment":"finalized","maxSupportedTransactionVersion":1}]});
            let raw = self.post(&mut state, &request, false)?;
            let status = if _11_lookup::missing(&raw, false)? {
                let request = _11_lookup::status_request(signature);
                let bytes = self.post(&mut state, &request, false)?;
                Some((request, bytes))
            } else {
                None
            };
            let target_slot = match &status {
                Some((_, bytes)) => _11_lookup::status_slot(bytes)?,
                None => transaction_slot(&raw, signature)?,
            };
            let coverage = &self.identity["coverage"];
            require(
                target_slot >= slot(&coverage["firstSlot"], "SOURCE_INTEGRITY", "first slot")?
                    && target_slot <= slot(&coverage["lastSlot"], "SOURCE_INTEGRITY", "last slot")?,
                "SOURCE_UNAVAILABLE",
                "transaction outside configured coverage",
            )?;
            let mut value = json!({"signature":signature,"slot":target_slot,"request":request,
                "responseSha256":Digest::of(&raw),"responseBodyBase64":STANDARD.encode(&raw),
                "genesisResponseBodyBase64":STANDARD.encode(state.genesis_raw.as_ref().ok_or_else(|| SourceError::new("SOURCE_INTEGRITY","discovery genesis missing"))?)});
            if let Some((request, bytes)) = status {
                value["lookupKind"] = json!("signature-status");
                value["statusRequest"] = request;
                value["statusResponseSha256"] = json!(Digest::of(&bytes));
                value["statusResponseBodyBase64"] = json!(STANDARD.encode(bytes));
            }
            Ok(value)
        })();
        if result.is_err() {
            self.state
                .lock()
                .map_err(|_| {
                    SourceError::new(
                        "SOURCE_INTEGRITY",
                        "source state poisoned while recording failure",
                    )
                })?
                .counters
                .failed_inspections += 1;
        }
        result
    }
}
