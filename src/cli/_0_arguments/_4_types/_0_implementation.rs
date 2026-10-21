use super::*;

#[derive(ClapArgs, Default, Clone)]
pub(crate) struct RuntimeArgs {
    /// Use this worker catalog (with --catalog-sha256).
    #[arg(long, value_name = "FILE", help_heading = help::PINNED, requires = "catalog_sha256", conflicts_with = "bundle")]
    pub(in super::super) catalog: Option<PathBuf>,
    /// Trusted SHA-256 of --catalog.
    #[arg(long, value_name = "SHA256", help_heading = help::PINNED, requires = "catalog")]
    pub(in super::super) catalog_sha256: Option<String>,
    /// Use this installed bundle.json (with --bundle-sha256).
    #[arg(long, value_name = "FILE", help_heading = help::PINNED, requires = "bundle_sha256", conflicts_with = "catalog")]
    pub(in super::super) bundle: Option<PathBuf>,
    /// Trusted SHA-256 of --bundle.
    #[arg(long, value_name = "SHA256", help_heading = help::PINNED, requires = "bundle")]
    pub(in super::super) bundle_sha256: Option<String>,
}

impl RuntimeArgs {
    pub(in super::super) fn resolve(self) -> Result<(PathBuf, Digest), Error> {
        match (
            self.catalog,
            self.catalog_sha256,
            self.bundle,
            self.bundle_sha256,
        ) {
            (Some(path), Some(pin), None, None) => Ok((path, Digest::new(pin)?)),
            (None, None, None, None) => signature::installation::catalog(),
            (None, None, Some(path), Some(pin)) => {
                let installed = super::super::super::bundle::open(&path, &Digest::new(pin)?)?;
                let exe =
                    std::env::current_exe().map_err(|e| Error::new("BUNDLE_IO", e.to_string()))?;
                if file_sha256(&exe).map_err(|e| Error::new("BUNDLE_IO", e.to_string()))?
                    != installed.binary_sha256.as_str()
                {
                    return Err(Error::new(
                        "BUNDLE_IDENTITY",
                        "run the CLI included in this bundle",
                    ));
                }
                Ok((installed.catalog, installed.catalog_sha256))
            }
            _ => Err(Error::new(
                "INVALID_REQUEST",
                "supply a pinned --catalog or --bundle",
            )),
        }
    }
}

#[derive(Subcommand)]
pub(in super::super) enum BundleCommand {
    /// Copy the running CLI and reviewed workers into a new offline directory.
    Pack {
        /// Worker catalog to pack (with --catalog-sha256).
        #[arg(long, value_name = "FILE")]
        catalog: PathBuf,
        /// Trusted SHA-256 of --catalog.
        #[arg(long, value_name = "SHA256")]
        catalog_sha256: String,
        /// The new directory to create.
        #[arg(long, value_name = "DIR")]
        output: PathBuf,
        /// Include only these reviewed families (repeat or comma-separate).
        #[arg(long, value_delimiter = ',')]
        family: Vec<String>,
        /// Omit optional trace/debug workers; requesting those modes then rejects.
        #[arg(long)]
        reference_only: bool,
    },
    /// Install an independently pinned offline directory without overwriting data.
    Install {
        /// The bundle.json to install (with --bundle-sha256).
        #[arg(long, value_name = "FILE")]
        bundle: PathBuf,
        /// Trusted SHA-256 of --bundle.
        #[arg(long, value_name = "SHA256")]
        bundle_sha256: String,
        /// Where to install; existing data is never overwritten.
        #[arg(long, value_name = "DIR")]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub(in super::super) enum CacheChoice {
    Off,
    Prepared,
    All,
}

#[derive(Parser)]
#[command(
    name = "svm-replay",
    version,
    about = "Replay Solana transactions, execute payloads, or fork historical state"
)]
pub(in super::super) struct Args {
    #[command(flatten)]
    pub(in super::super) signature: signature::Options,
    /// Replay the bundled example instead: offline, no key needed.
    #[arg(long, help_heading = help::REPLAY, conflicts_with = "tx")]
    pub(in super::super) demo: bool,
    /// Print compact JSON and never show menus.
    #[arg(long, global = true, help_heading = help::OUTPUT, display_order = 1, conflicts_with = "human")]
    pub(in super::super) json: bool,
    /// Indent the JSON, also when piped or saved with --out.
    #[arg(long, global = true, help_heading = help::OUTPUT, display_order = 2, conflicts_with = "human")]
    pub(in super::super) pretty: bool,
    /// Print a readable summary instead of JSON.
    #[arg(long, global = true, help_heading = help::OUTPUT, display_order = 0)]
    pub(in super::super) human: bool,
    /// On an internal error, write an issue draft to FILE (never uploaded).
    #[arg(long, global = true, value_name = "FILE", help_heading = help::OUTPUT)]
    pub(in super::super) bug_report: Option<PathBuf>,
    /// Write progress events (svm-replay-progress/v1 NDJSON) to stderr, for UIs.
    #[arg(long, global = true, help_heading = help::OUTPUT)]
    pub(in super::super) progress: bool,
    #[command(subcommand)]
    pub(in super::super) command: Option<Command>,
}
