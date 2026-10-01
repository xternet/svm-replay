use super::*;

#[derive(Args, Default)]
pub(in super::super) struct Controls {
    /// Explicit provider coverage and budgets JSON (same contract as simulate --alchemy).
    #[arg(long, requires = "tx", conflicts_with_all = ["max_requests", "max_download_bytes"])]
    pub alchemy_config: Option<PathBuf>,
    /// Collect all, JSON or @file: calls, instructions, registers, memory and optional limits.
    #[arg(long, requires = "tx")]
    pub collect: Option<String>,
    /// Total acquisition/execution deadline in seconds (default 900, maximum 3600).
    #[arg(long, requires = "tx")]
    pub timeout: Option<u64>,
    /// Maximum live RPC requests, including discovery (default 5000).
    #[arg(long, requires = "tx")]
    pub max_requests: Option<u64>,
    /// Maximum downloaded bytes (default 536870912).
    #[arg(long, requires = "tx")]
    pub max_download_bytes: Option<u64>,
    /// Maximum worker result bytes (default 268435456).
    #[arg(long, requires = "tx")]
    pub max_output_bytes: Option<u64>,
    /// Bank evidence name in --source. Repeat for additional exact inputs.
    #[arg(long, requires_all = ["tx", "source"], value_parser = ["initializedStakeEvidence", "epochStakeEvidence", "initializationEvidence", "programMigrationEvidence"])]
    pub bank_input: Vec<String>,
    /// Additional captured historical source, never current-state fallback.
    #[arg(long, requires_all = ["tx", "source_sha256"])]
    pub source: Option<PathBuf>,
    #[arg(long, requires = "source")]
    pub source_sha256: Option<String>,
    /// Reviewed runtime registry JSON. Requires its independently trusted SHA256.
    #[arg(long, requires_all = ["tx", "runtime_registry_sha256"])]
    pub runtime_registry: Option<PathBuf>,
    #[arg(long, requires = "runtime_registry")]
    pub runtime_registry_sha256: Option<String>,
}
impl Controls {
    pub fn limits(&self) -> Result<Limits, Error> {
        let timeout_ms = self
            .timeout
            .unwrap_or(900)
            .checked_mul(1000)
            .ok_or_else(|| Error::new("INVALID_REQUEST", "timeout overflow"))?;
        let limits = Limits {
            timeout_ms,
            max_output_bytes: self.max_output_bytes.unwrap_or(256 * 1024 * 1024),
            max_diagnostic_bytes: 1024 * 1024,
        };
        if timeout_ms == 0 || self.max_requests == Some(0) || self.max_download_bytes == Some(0) {
            return Err(Error::new("INVALID_REQUEST", "budgets must be positive"));
        }
        limits.validate()?;
        Ok(limits)
    }
    pub fn registry(&self) -> Result<Registry, Error> {
        match (&self.runtime_registry, &self.runtime_registry_sha256) {
            (None, None) => Registry::bundled(),
            (Some(path), Some(pin)) => {
                let bytes = read_bounded_file(path, 16 * 1024 * 1024)
                    .map_err(|e| Error::new("INPUT_IO", e.to_string()))?;
                if Digest::of(&bytes) != Digest::new(pin.clone())? {
                    return Err(Error::new("RUNTIME_REGISTRY", "registry hash differs"));
                }
                Registry::from_bytes(&bytes)
            }
            _ => Err(Error::new(
                "INVALID_REQUEST",
                "registry path and pin required together",
            )),
        }
    }
}
pub(in super::super) fn remaining(start: Instant, limits: &Limits) -> Result<Duration, Error> {
    Duration::from_millis(limits.timeout_ms)
        .checked_sub(start.elapsed())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| Error::new("WORKER_TIMEOUT", "signature request deadline exceeded"))
}
