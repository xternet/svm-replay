use serde_json::{json, Value};
use std::{io::Write, net::TcpListener, path::PathBuf, time::Duration};
use svm_replay_engine::shared::{
    historical::{
        recorded_journal_input, require_journal, serve_backend, serve_session, JournalFile,
        SessionInput, VerifiedSession,
    },
    runtime::{
        file_sha256, protocol_error, read_bounded_file, ExecutionBudget, ProcessOwner,
        WorkerLimits, WorkerTransport,
    },
};
use svm_replay_protocol::{parse_json, Error};
mod _0_implementation;
mod _1_parent;
pub(super) use _0_implementation::run;
#[cfg(test)]
mod tests;

#[derive(clap::Args)]
pub(crate) struct Options {
    /// Pinned local session descriptor JSON; no credentials.
    #[arg(
        long,
        value_name = "FILE",
        group = "replay_target",
        required_unless_present = "after",
        conflicts_with_all = [
            "after", "HistoricalProviderOptions", "alchemy_config", "collect",
            "max_requests", "max_download_bytes", "max_output_bytes", "bank_input",
            "source", "source_sha256", "runtime_registry", "runtime_registry_sha256",
            "catalog", "catalog_sha256", "bundle", "bundle_sha256"
        ]
    )]
    request: Option<PathBuf>,
    /// Start a frozen historical fork immediately after this transaction or slot.
    #[arg(long, value_name = "SIGNATURE|SLOT", group = "replay_target")]
    after: Option<String>,
    /// Loopback HTTP port (WebSocket on port + 1); zero selects a free pair.
    #[arg(long, default_value_t = 0)]
    port: u16,
    /// Bind this session's lifetime to its actual SDK parent (Linux/macOS).
    #[arg(long, hide = true)]
    parent_pid: Option<u32>,
    /// Emit measured local transaction events on stdout (JSONL).
    #[arg(long, requires = "json")]
    events: bool,
    /// Serve the session to an embedding tool over stdin/stdout instead of loopback RPC.
    #[arg(long, value_parser = ["stdio"], hide = true, conflicts_with_all = ["port", "events"])]
    backend: Option<String>,
    /// Journal the backend's committed operations in FILE and restore them on restart.
    #[arg(long, value_name = "FILE", hide = true, requires = "backend")]
    journal: Option<PathBuf>,
    /// The embedder's store identity the journal belongs to.
    #[arg(long, value_name = "ID", hide = true, requires = "journal")]
    journal_binding: Option<String>,
    /// Whether the embedder's store has history (`existing`: the journal must exist) or not
    /// (`new`: it must not).
    #[arg(long, value_parser = ["existing", "new"], hide = true, requires = "journal")]
    journal_require: Option<String>,
    /// Keep strict worker scratch in DIR.
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    #[command(flatten)]
    historical: crate::arguments::HistoricalOptions,
    #[command(flatten)]
    controls: crate::arguments::SignatureControls,
    #[command(flatten)]
    runtime: crate::arguments::RuntimeArgs,
}
