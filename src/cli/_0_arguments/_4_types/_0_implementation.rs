use super::*;

#[derive(ClapArgs, Default, Clone)]
pub(in super::super) struct RuntimeArgs {
    #[arg(long, requires = "catalog_sha256", conflicts_with = "bundle")]
    pub(in super::super) catalog: Option<PathBuf>,
    #[arg(long, requires = "catalog")]
    pub(in super::super) catalog_sha256: Option<String>,
    #[arg(long, requires = "bundle_sha256", conflicts_with = "catalog")]
    pub(in super::super) bundle: Option<PathBuf>,
    #[arg(long, requires = "bundle")]
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
        #[arg(long)]
        catalog: PathBuf,
        #[arg(long)]
        catalog_sha256: String,
        #[arg(long)]
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
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        bundle_sha256: String,
        #[arg(long)]
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
    about = "Historical Solana simulation and trace export"
)]
pub(in super::super) struct Args {
    #[command(flatten)]
    pub(in super::super) signature: signature::Options,
    /// Run the bundled offline example without an API key.
    #[arg(long, conflicts_with = "tx")]
    pub(in super::super) demo: bool,
    /// Force compact JSON and disable menus; --pretty overrides indentation.
    #[arg(long, global = true, conflicts_with = "human")]
    pub(in super::super) json: bool,
    /// Force indented JSON, including pipes and --out files (terminal JSON is indented by default).
    #[arg(long, global = true, conflicts_with = "human")]
    pub(in super::super) pretty: bool,
    /// Print a readable summary instead of JSON (not for experimental JSONL sessions).
    #[arg(long, global = true)]
    pub(in super::super) human: bool,
    /// Write a safe local issue draft on a typed INTERNAL error; never uploads.
    #[arg(long, global = true)]
    pub(in super::super) bug_report: Option<PathBuf>,
    #[command(subcommand)]
    pub(in super::super) command: Option<Command>,
}
