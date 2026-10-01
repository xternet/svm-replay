use super::*;

impl AlchemySource {
    pub(in super::super) fn post(
        &self,
        state: &mut State,
        request: &Value,
        account: bool,
    ) -> Result<Vec<u8>> {
        self.check_cancelled()?;
        self.remaining()?;
        for (name, used, limit, applies) in [
            (
                "requests",
                state.counters.requests,
                self.limits.max_requests,
                true,
            ),
            (
                "accountReads",
                state.counters.account_reads,
                self.limits.max_account_reads,
                account,
            ),
            (
                "downloadBytes",
                state.counters.charged_bytes,
                self.limits.max_download_bytes,
                true,
            ),
        ] {
            if applies && used >= limit {
                return Err(SourceError::new("SOURCE_RESOURCE_LIMIT",
                    format!("historical acquisition budget exceeded: {name} {used}/{limit}; increase the explicit provider budget to continue"))
                    .details(json!({"reason":"BUDGET_EXCEEDED","limitName":name,"used":used,"limit":limit})));
            }
        }
        let mut max = self
            .limits
            .max_response_bytes
            .min(self.limits.max_download_bytes - state.counters.charged_bytes);
        let calls = match request.as_array() {
            Some(batch) => batch.as_slice(),
            None => std::slice::from_ref(request),
        };
        let methods = calls
            .iter()
            .map(|call| {
                call["method"].as_str().map(str::to_owned).ok_or_else(|| {
                    SourceError::new(
                        "SOURCE_INTEGRITY",
                        "RPC method missing for request accounting",
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let request = serde_json::to_vec(request).map_err(|_| {
            SourceError::new("SOURCE_INTEGRITY", "RPC request serialization failed")
        })?;
        let mut reserved = 0;
        if let Some(budget) = &self.budget {
            max = budget.reserve(account, max)?;
            reserved = max.checked_add(1).ok_or_else(|| {
                SourceError::new("SOURCE_RESOURCE_LIMIT", "response reservation overflow")
            })?;
            state.counters.charged_bytes = state
                .counters
                .charged_bytes
                .checked_add(reserved)
                .ok_or_else(|| {
                    SourceError::new("SOURCE_RESOURCE_LIMIT", "charged byte accounting overflow")
                })?;
        }
        // A durable commit may wait on SQLite; never reuse its earlier timeout.
        let remaining = self.remaining()?;
        state.counters.requests += 1;
        for method in methods {
            *state.counters.methods.entry(method).or_insert(0) += 1;
        }
        if account {
            state.counters.account_reads += 1;
        }
        let response = match self.transport.post(
            &request,
            remaining.min(Duration::from_secs(30)),
            max,
        ) {
            Ok(response) => response,
            Err(failure) => {
                // A failed transport cannot promise how many partial bytes arrived.
                // Retain the full reservation so failures cannot reset a byte budget.
                if self.budget.is_none() {
                    state.counters.charged_bytes += max;
                }
                self.check_cancelled()?;
                return Err(match failure {
                    TransportFailure::Timeout => {
                        SourceError::new("SOURCE_DEADLINE", "historical RPC deadline exceeded")
                    }
                    TransportFailure::ResponseTooLarge => SourceError::new(
                        "SOURCE_RESOURCE_LIMIT",
                        "historical RPC response exceeded byte limit",
                    ),
                    TransportFailure::Network => {
                        SourceError::new("SOURCE_TRANSPORT", "historical RPC transport failed")
                    }
                    TransportFailure::CredentialEcho => SourceError::new(
                        "SOURCE_INTEGRITY",
                        "provider response rejected by credential redaction guard",
                    ),
                }
                .details(json!({"chargedBytes":if self.budget.is_some(){reserved}else{max},"downloadedBytesKnown":false})));
            }
        };
        state.counters.downloaded_bytes = state
            .counters
            .downloaded_bytes
            .checked_add(response.body.len() as u64)
            .ok_or_else(|| {
                SourceError::new("SOURCE_RESOURCE_LIMIT", "download accounting overflow")
            })?;
        let extra = if self.budget.is_some() {
            (response.body.len() as u64).saturating_sub(reserved)
        } else {
            response.body.len() as u64
        };
        state.counters.charged_bytes =
            state
                .counters
                .charged_bytes
                .checked_add(extra)
                .ok_or_else(|| {
                    SourceError::new("SOURCE_RESOURCE_LIMIT", "charged byte accounting overflow")
                })?;
        if let Some(budget) = &self.budget {
            if extra > 0 {
                budget.observe_excess(extra)?;
            }
        }
        self.check_cancelled()?;
        require(
            response.body.len() as u64 <= max
                && state.counters.charged_bytes <= self.limits.max_download_bytes,
            "SOURCE_RESOURCE_LIMIT",
            "historical RPC response exceeded byte limit",
        )?;
        require(
            Instant::now() < self.expires,
            "SOURCE_DEADLINE",
            "historical RPC completed after deadline",
        )?;
        if !(200..300).contains(&response.status) {
            let message = match response.status {
                401 => "Alchemy rejected authentication; check API_ALCHEMY or the selected key file",
                402 => "Alchemy rejected payment; check account billing and available credits",
                403 => "Alchemy denied access; check key restrictions and Solana Account Archive access",
                429 => "Alchemy rate or usage limit reached; check throughput limits and remaining credits",
                _ => "historical RPC HTTP failure",
            };
            let code = if response.status == 429 {
                "SOURCE_RATE_LIMIT"
            } else {
                "SOURCE_HTTP_ERROR"
            };
            return Err(SourceError::new(code, message).details(json!({
                "httpStatus":response.status,"responseBytes":response.body.len(),
                "responseSha256":Digest::of(&response.body)
            })));
        }
        Ok(response.body)
    }
}
