use super::*;
use crate::arguments::help;

#[derive(Args, Default)]
pub(crate) struct Controls {
    /// Provider coverage and budgets as JSON (not with --max-requests or --max-download-bytes).
    #[arg(long, value_name = "FILE", help_heading = help::PROVIDER, requires = "replay_target", conflicts_with_all = ["max_requests", "max_download_bytes"])]
    pub alchemy_config: Option<PathBuf>,
    /// Also record a trace: all, or JSON/@FILE choosing calls, instructions, registers, memory.
    #[arg(long, value_name = "WHAT", help_heading = help::OUTPUT, requires = "replay_target",
        long_help = "Also record a trace: all, or JSON/@FILE choosing calls, instructions, registers,\n\
                     memory and optional limits (see docs/tracing.md).")]
    pub collect: Option<String>,
    /// Stop after this many seconds (default 900, at most 3600).
    #[arg(long, value_name = "SECONDS", help_heading = help::LIMITS, requires = "replay_target")]
    pub timeout: Option<u64>,
    /// At most this many live provider requests, discovery included (default 5000).
    #[arg(long, value_name = "N", help_heading = help::LIMITS, requires = "replay_target")]
    pub max_requests: Option<u64>,
    /// At most this many downloaded bytes (default 536870912, 512 MiB).
    #[arg(long, value_name = "BYTES", help_heading = help::LIMITS, requires = "replay_target")]
    pub max_download_bytes: Option<u64>,
    /// At most this many result bytes (default 268435456, 256 MiB).
    #[arg(long, value_name = "BYTES", help_heading = help::LIMITS, requires = "replay_target")]
    pub max_output_bytes: Option<u64>,
    /// Bank evidence to take from --source (repeatable).
    #[arg(long, value_name = "NAME", help_heading = help::PINNED, requires_all = ["replay_target", "source"], value_parser = ["initializedStakeEvidence", "epochStakeEvidence", "initializationEvidence", "programMigrationEvidence"])]
    pub bank_input: Vec<String>,
    /// Extra captured historical source (with --source-sha256); never today's state.
    #[arg(long, value_name = "FILE", help_heading = help::PINNED, requires_all = ["replay_target", "source_sha256"])]
    pub source: Option<PathBuf>,
    /// Trusted SHA-256 of --source.
    #[arg(long, value_name = "SHA256", help_heading = help::PINNED, requires = "source")]
    pub source_sha256: Option<String>,
    /// Reviewed runtime registry (with --runtime-registry-sha256).
    #[arg(long, value_name = "FILE", help_heading = help::PINNED, requires_all = ["replay_target", "runtime_registry_sha256"])]
    pub runtime_registry: Option<PathBuf>,
    /// Trusted SHA-256 of --runtime-registry.
    #[arg(long, value_name = "SHA256", help_heading = help::PINNED, requires = "runtime_registry")]
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
