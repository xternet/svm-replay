use super::*;

#[derive(Subcommand)]
pub(in super::super) enum Command {
    /// Serve a frozen historical Solana RPC fork.
    Fork {
        #[command(flatten)]
        options: crate::_11_session::Options,
    },
    /// Run a prepared historical session (legacy advanced interface).
    #[command(hide = true)]
    Session {
        #[command(flatten)]
        options: crate::_11_session::Options,
    },
    /// Export a historical fork boundary from pinned replay jobs (advanced, offline).
    Boundary {
        #[arg(long, value_name = "FILE")]
        request: PathBuf,
    },
    /// Replay the bundled example offline (no key or network).
    Demo {
        /// Select the bundled example without prompting.
        #[arg(long, value_parser = ["default"])]
        example: Option<String>,
        /// Use this installed bundle.json (with --bundle-sha256).
        #[arg(long, value_name = "FILE", requires = "bundle_sha256")]
        bundle: Option<PathBuf>,
        /// Trusted SHA-256 of --bundle.
        #[arg(long, value_name = "SHA256", requires = "bundle")]
        bundle_sha256: Option<String>,
        /// Keep runs in DIR instead of the default data folder.
        #[arg(long, value_name = "DIR")]
        data_dir: Option<PathBuf>,
    },
    /// Check the installation (hashes and host); nothing runs or downloads.
    Doctor {
        #[command(flatten)]
        runtime: RuntimeArgs,
    },
    /// List installed runtimes and coverage; nothing runs or downloads.
    #[command(name = "runtimes", alias = "capabilities")]
    Capabilities {
        #[command(flatten)]
        runtime: RuntimeArgs,
    },
    /// Generate shell completions.
    Completions {
        #[arg(value_enum, value_name = "SHELL")]
        shell: clap_complete::Shell,
    },
    /// Run a reviewed historical request file (advanced).
    Simulate {
        /// The request file (prepared or historical).
        #[arg(long, value_name = "FILE")]
        request: PathBuf,
        #[command(flatten)]
        runtime: RuntimeArgs,
        /// Keep runs and the cache in DIR instead of the default data folder.
        #[arg(long, value_name = "DIR")]
        data_dir: Option<PathBuf>,
        /// Reuse inputs and prepared boundaries (prepared), and results too (all).
        #[arg(long, value_enum, default_value = "off")]
        cache: CacheChoice,
        /// Extra captured historical source (with --source-sha256).
        #[arg(long, value_name = "FILE", requires = "source_sha256")]
        source: Option<PathBuf>,
        /// Trusted SHA-256 of --source.
        #[arg(long, value_name = "SHA256", requires = "source")]
        source_sha256: Option<String>,
        /// JSON capture request and producer bounds; requires a pinned capture worker.
        #[arg(long, value_name = "FILE")]
        trace: Option<PathBuf>,
        /// Experimental: stream debugger JSONL for a fully prepared boundary.
        #[arg(long, requires = "trace", hide = true)]
        debug: bool,
        /// Explicit provider coverage/budget JSON; reads only API_ALCHEMY.
        #[arg(long, value_name = "FILE")]
        alchemy: Option<PathBuf>,
        /// Required network intent when hydrating a legacy prepared request.
        #[arg(long, value_name = "HASH")]
        genesis_hash: Option<String>,
    },
    /// Check a request file and print its identity; nothing is fetched or run (advanced).
    Validate {
        /// The request file.
        #[arg(long, value_name = "FILE")]
        request: PathBuf,
    },
    /// Build a request file from reviewed inputs (advanced).
    Request {
        /// Reviewed candidate: slot/transaction boundary, block identity and runtime binding.
        #[arg(long, value_name = "FILE", required_unless_present_any = ["block", "alchemy"], conflicts_with_all = ["block", "alchemy"])]
        candidate: Option<PathBuf>,
        /// Full archived getBlock JSON; derives the boundary from --signature.
        #[arg(
            long,
            value_name = "FILE",
            requires = "signature",
            conflicts_with = "alchemy"
        )]
        block: Option<PathBuf>,
        /// The transaction (with --block or --alchemy).
        #[arg(long, value_name = "SIGNATURE")]
        signature: Option<String>,
        /// Fetch the block at the reviewed binding's slot with explicit provider limits.
        #[arg(long, value_name = "FILE", requires_all = ["signature", "source_output"])]
        alchemy: Option<PathBuf>,
        /// Save acquisition provenance; existing files are never overwritten.
        #[arg(long, value_name = "FILE", requires = "alchemy")]
        source_output: Option<PathBuf>,
        /// Keep the cache in DIR instead of the default data folder.
        #[arg(long, value_name = "DIR")]
        data_dir: Option<PathBuf>,
        /// Runtime binding reviewed for this slot.
        #[arg(long, value_name = "FILE")]
        runtime_binding: PathBuf,
        /// Family ID of a pinned runtime, such as v3-0 (see docs/runtimes.md).
        #[arg(long, value_name = "ID")]
        family: String,
        /// Genesis hash of the network.
        #[arg(long, value_name = "HASH")]
        genesis_hash: String,
        /// An ID of your choice for this request.
        #[arg(long, value_name = "ID")]
        request_id: String,
        /// Where to write the request; an existing file is never overwritten.
        #[arg(long, value_name = "FILE")]
        output: PathBuf,
        /// Run this transaction (base64) instead of the original.
        #[arg(long, value_name = "BASE64")]
        replacement_base64: Option<String>,
        /// Account and program overrides (JSON file).
        #[arg(long, value_name = "FILE")]
        overrides: Option<PathBuf>,
        /// Bank evidence to include (repeatable).
        #[arg(long, value_name = "NAME")]
        bank_input: Vec<String>,
        /// Execution deadline in milliseconds.
        #[arg(long, value_name = "MS", default_value_t = 180000)]
        timeout_ms: u64,
        /// At most this many result bytes.
        #[arg(long, value_name = "BYTES", default_value_t = 268435456)]
        max_output_bytes: u64,
        /// At most this many diagnostic bytes.
        #[arg(long, value_name = "BYTES", default_value_t = 1048576)]
        max_diagnostic_bytes: u64,
    },
    /// Pack or install an offline, hash-pinned bundle (advanced).
    Bundle {
        #[command(subcommand)]
        command: BundleCommand,
    },
}
