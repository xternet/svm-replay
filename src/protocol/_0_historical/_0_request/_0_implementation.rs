use super::*;

/// A reviewed historical boundary and runtime, not an automatically inferred era.
/// Sources are supplied separately so secrets and machine paths never enter this request.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoricalRequest {
    pub schema: String,
    pub request_id: String,
    pub family: String,
    pub genesis_hash: String,
    pub candidate: Value,
    pub runtime_binding: Value,
    pub replacement_transaction_base64: Option<String>,
    pub requested_account_overrides: Option<Value>,
    pub bank_inputs: Vec<String>,
    pub metadata_policy: MetadataPolicy,
    pub limits: Limits,
}

impl HistoricalRequest {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let value = parse_json(bytes)?;
        validate_safe_numbers(&value)?;
        let input: Self = serde_json::from_value(value)
            .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
        input.validate()?;
        Ok(input)
    }
    pub fn validate(&self) -> Result<(), Error> {
        let invalid = |message: &str| Error::new("INVALID_REQUEST", message);
        self.limits.validate()?;
        if self.schema != "svm-replay-historical/v1"
            || self.request_id.is_empty()
            || self.request_id.len() > 200
            || !self
                .request_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        {
            return Err(invalid("invalid historical schema/requestId"));
        }
        if !["v2-2", "v2-3", "v3-0", "v3-1", "v4-0", "v4-1", "v4-2"].contains(&self.family.as_str())
        {
            return Err(Error::new(
                "UNSUPPORTED_RUNTIME",
                "runtime family is not reviewed",
            ));
        }
        let genesis = bs58::decode(&self.genesis_hash)
            .into_vec()
            .map_err(|_| invalid("invalid genesis"))?;
        if genesis.len() != 32 || bs58::encode(genesis).into_string() != self.genesis_hash {
            return Err(invalid("invalid genesis"));
        }
        let slot = self.candidate["slot"]
            .as_u64()
            .filter(|n| *n <= 9_007_199_254_740_991)
            .ok_or_else(|| invalid("historical slot required"))?;
        self.candidate["transactionIndex"]
            .as_u64()
            .filter(|n| *n <= 9_007_199_254_740_991)
            .ok_or_else(|| invalid("historical transaction index required"))?;
        Digest::new(
            self.candidate["blockSourceHash"]
                .as_str()
                .ok_or_else(|| invalid("raw block pin required"))?,
        )?;
        let binding = &self.runtime_binding;
        crate::runtime::validate_runtime_binding(binding)?;
        for key in ["runtimeProfileId", "executorSourceId", "litesvmCommit"] {
            if self.candidate[key].as_str().is_none_or(str::is_empty) {
                return Err(invalid("candidate runtime identity missing"));
            }
        }
        if binding["targetSlot"].as_u64() != Some(slot)
            || binding["runtimeProfileId"] != self.candidate["runtimeProfileId"]
            || binding["executor"]["id"] != self.candidate["executorSourceId"]
            || binding["executor"]["litesvmCommit"] != self.candidate["litesvmCommit"]
        {
            return Err(Error::new("WORKER_IDENTITY", "candidate/runtime mismatch"));
        }
        let active = self.candidate["activeFeatures"]
            .as_array()
            .ok_or_else(|| invalid("active features required"))?;
        let features = binding["features"]
            .as_array()
            .ok_or_else(|| invalid("feature activations required"))?;
        let mut seen = std::collections::BTreeSet::new();
        if active.len() != features.len() {
            return Err(invalid("feature list differs"));
        }
        for (feature, expected) in features.iter().zip(active) {
            let id = expected
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| invalid("invalid feature"))?;
            if feature["id"] != id
                || !seen.insert(id)
                || feature["activationSlot"].as_u64().is_none_or(|n| n > slot)
            {
                return Err(invalid("feature list/activation differs"));
            }
        }
        let mut names = std::collections::BTreeSet::new();
        for name in &self.bank_inputs {
            if ![
                "initializedStakeEvidence",
                "epochStakeEvidence",
                "initializationEvidence",
                "programMigrationEvidence",
            ]
            .contains(&name.as_str())
                || !names.insert(name.as_str())
            {
                return Err(invalid("unknown/duplicate Bank input"));
            }
        }
        if names.contains("initializedStakeEvidence") && names.contains("initializationEvidence") {
            return Err(invalid(
                "exact initialized stakes and recovery evidence are mutually exclusive",
            ));
        }
        if let Some(wire) = &self.replacement_transaction_base64 {
            crate::transaction::decode(wire)?;
        }
        if self
            .requested_account_overrides
            .as_ref()
            .is_some_and(|v| !v.is_array())
        {
            return Err(invalid("requested overrides must be an array"));
        }
        Ok(())
    }
}
