use super::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    pub timeout_ms: u64,
    pub max_output_bytes: u64,
    pub max_diagnostic_bytes: u64,
}

impl Limits {
    pub fn validate(&self) -> Result<(), Error> {
        for (name, value, max) in [
            ("timeoutMs", self.timeout_ms, 3_600_000),
            ("maxOutputBytes", self.max_output_bytes, 256 * 1024 * 1024),
            ("maxDiagnosticBytes", self.max_diagnostic_bytes, 1024 * 1024),
        ] {
            if value == 0 || value > max {
                return Err(Error::new(
                    "INVALID_LIMIT",
                    format!("{name} must be between 1 and {max}"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum MetadataPolicy {
    #[serde(rename = "STRICT")]
    Strict,
    #[serde(rename = "ARCHIVED_COMPUTE_METER_WARNING")]
    ArchivedComputeMeterWarning,
}

/// Explicit supplied boundary. Use HistoricalRequest for reconstruction from sources.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedRequest {
    pub schema: String,
    pub request_id: String,
    pub family: String,
    pub candidate: Value,
    pub fixture: Value,
    pub raw_block_base64: String,
    pub block_sha256: Digest,
    pub source_evidence_hashes: Vec<Digest>,
    pub metadata_policy: MetadataPolicy,
    pub limits: Limits,
}

impl PreparedRequest {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let value = parse_json(bytes)?;
        validate_safe_numbers(&value)?;
        let request: Self = serde_json::from_value(value)
            .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
        request.validate()?;
        Ok(request)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != "svm-replay-prepared/v1" {
            return Err(Error::new(
                "INVALID_REQUEST",
                "expected svm-replay-prepared/v1",
            ));
        }
        if self.request_id.is_empty()
            || self.request_id.len() > 200
            || !self
                .request_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        {
            return Err(Error::new("INVALID_REQUEST", "invalid requestId"));
        }
        if !["v2-2", "v2-3", "v3-0", "v3-1", "v4-0", "v4-1", "v4-2"].contains(&self.family.as_str())
        {
            return Err(Error::new(
                "UNSUPPORTED_RUNTIME",
                "runtime family is not reviewed",
            ));
        }
        if !self.candidate.is_object()
            || !self.fixture.is_object()
            || self.source_evidence_hashes.is_empty()
        {
            return Err(Error::new(
                "INVALID_REQUEST",
                "candidate, fixture and source evidence are required",
            ));
        }
        self.limits.validate()?;
        let bytes = STANDARD
            .decode(&self.raw_block_base64)
            .map_err(|e| Error::new("BLOCK_INTEGRITY", e.to_string()))?;
        if STANDARD.encode(&bytes) != self.raw_block_base64
            || Digest::of(&bytes) != self.block_sha256
            || self
                .candidate
                .get("blockSourceHash")
                .and_then(Value::as_str)
                != Some(self.block_sha256.as_str())
        {
            return Err(Error::new(
                "BLOCK_INTEGRITY",
                "raw block/candidate evidence hashes differ",
            ));
        }
        self.archived_block()?;
        Ok(())
    }
    pub fn archived_block(&self) -> Result<Value, Error> {
        let bytes = STANDARD
            .decode(&self.raw_block_base64)
            .map_err(|e| Error::new("BLOCK_INTEGRITY", e.to_string()))?;
        parse_json(&bytes)
    }
}
