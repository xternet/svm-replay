use super::*;

#[derive(Subcommand)]
pub(in super::super) enum Command {
    /// Execute the installed, pinned demo inputs afresh. No provider or network.
    Demo {
        /// Select the bundled example without prompting.
        #[arg(long, value_parser = ["default"])]
        example: Option<String>,
        #[arg(long, requires = "bundle_sha256")]
        bundle: Option<PathBuf>,
        #[arg(long, requires = "bundle")]
        bundle_sha256: Option<String>,
        #[arg(long)]
        data_dir: Option<PathBuf>,
    },
    /// Reconstruct or execute an explicitly reviewed historical request.
    Simulate {
        #[arg(long)]
        request: PathBuf,
        #[command(flatten)]
        runtime: RuntimeArgs,
        #[arg(long)]
        data_dir: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "off")]
        cache: CacheChoice,
        #[arg(long, requires = "source_sha256")]
        source: Option<PathBuf>,
        #[arg(long, requires = "source")]
        source_sha256: Option<String>,
        /// JSON capture request and producer bounds; requires a pinned capture worker.
        #[arg(long)]
        trace: Option<PathBuf>,
        /// Experimental: stream debugger JSONL for a fully prepared boundary.
        #[arg(long, requires = "trace", hide = true)]
        debug: bool,
        /// Explicit provider coverage/budget JSON; reads only API_ALCHEMY.
        #[arg(long)]
        alchemy: Option<PathBuf>,
        /// Required network intent when hydrating a legacy prepared request.
        #[arg(long)]
        genesis_hash: Option<String>,
    },
    /// Inspect qualified installed workers; does not execute or fetch history.
    Capabilities {
        #[command(flatten)]
        runtime: RuntimeArgs,
    },
    /// Verify installation hashes and host compatibility without execution.
    Doctor {
        #[command(flatten)]
        runtime: RuntimeArgs,
    },
    /// Validate a request and report its canonical identity without fetching or execution.
    Validate {
        #[arg(long)]
        request: PathBuf,
    },
    /// Build a STRICT historical request from explicitly reviewed inputs.
    Request {
        #[arg(long, required_unless_present_any = ["block", "alchemy"], conflicts_with_all = ["block", "alchemy"])]
        candidate: Option<PathBuf>,
        /// Full archived getBlock JSON; derives the boundary from --signature.
        #[arg(long, requires = "signature", conflicts_with = "alchemy")]
        block: Option<PathBuf>,
        #[arg(long)]
        signature: Option<String>,
        /// Fetch the block at the reviewed binding's slot with explicit provider limits.
        #[arg(long, requires_all = ["signature", "source_output"])]
        alchemy: Option<PathBuf>,
        /// Save acquisition provenance; existing files are never overwritten.
        #[arg(long, requires = "alchemy")]
        source_output: Option<PathBuf>,
        #[arg(long)]
        data_dir: Option<PathBuf>,
        #[arg(long)]
        runtime_binding: PathBuf,
        #[arg(long)]
        family: String,
        #[arg(long)]
        genesis_hash: String,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        replacement_base64: Option<String>,
        #[arg(long)]
        overrides: Option<PathBuf>,
        #[arg(long)]
        bank_input: Vec<String>,
        #[arg(long, default_value_t = 180000)]
        timeout_ms: u64,
        #[arg(long, default_value_t = 268435456)]
        max_output_bytes: u64,
        #[arg(long, default_value_t = 1048576)]
        max_diagnostic_bytes: u64,
    },
    /// Create or install offline, hash-pinned CLI/worker packages.
    Bundle {
        #[command(subcommand)]
        command: BundleCommand,
    },
}
