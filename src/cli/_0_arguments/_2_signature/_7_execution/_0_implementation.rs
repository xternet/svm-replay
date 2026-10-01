use super::*;

#[derive(Args, Default)]
pub(in super::super::super) struct Options {
    #[command(flatten)]
    pub(in super::super) runtime: super::super::super::RuntimeArgs,
    #[command(flatten)]
    pub(in super::super) controls: controls::Controls,
    /// Historical transaction signature. Fetch, reconstruct and replay automatically.
    #[arg(long)]
    pub tx: Option<String>,
    /// Replacement JSON or @path/to/payload.json, containing transactionBase64.
    #[arg(long, requires = "tx")]
    pub(in super::super) replace: Option<String>,
    /// Account/program override JSON array or @path/to/accounts.json.
    #[arg(long, requires = "tx")]
    pub(in super::super) overrides: Option<String>,
    /// Select result fields (comma-separated); errors and verification remain visible.
    #[arg(long, requires = "tx", value_delimiter = ',')]
    pub(in super::super) fields: Vec<String>,
    /// Save the full selected JSON as DIRECTORY/timestamp-signature.json.
    #[arg(long, requires = "tx")]
    pub(in super::super) out: Option<PathBuf>,
    /// Fetch historical inputs without reading or populating the reusable cache.
    #[arg(long, requires = "tx")]
    pub(in super::super) no_cache: bool,
    /// Read the Alchemy key from this local text file instead of API_ALCHEMY.
    #[arg(long, requires = "tx")]
    pub(in super::super) alchemy_key_file: Option<PathBuf>,
    /// Override the OS-specific application-data directory.
    #[arg(long, requires = "tx")]
    pub(in super::super) data_dir: Option<PathBuf>,
}

pub(in super::super::super) fn run(options: Options) -> Result<Value, Error> {
    let receipt = match execute(&options) {
        Ok(receipt) => receipt,
        Err(error) => super::super::super::error_receipt(error),
    };
    output::finish(
        receipt,
        options.tx.as_deref().unwrap_or(""),
        &options.fields,
        options.out.as_deref(),
    )
}

pub(in super::super) fn execute(options: &Options) -> Result<Value, Error> {
    let started = Instant::now();
    let cancellation = crate::lifecycle::cancellation::install()?;
    let limits = options.controls.limits()?;
    let signature = options
        .tx
        .as_deref()
        .ok_or_else(|| Error::new("INVALID_REQUEST", "--tx is required"))?;
    svm_replay_engine::shared::history::address(signature, 64)?;
    output::validate_fields(&options.fields)?;
    let trace = options
        .controls
        .collect
        .as_deref()
        .map(inputs::collect)
        .transpose()?
        .flatten();
    let tracing = trace.is_some();
    if options.fields.iter().any(|field| field == "trace") && !tracing {
        return Err(Error::new(
            "INVALID_REQUEST",
            "--fields trace requires an enabled --collect category",
        ));
    }
    let replacement = options
        .replace
        .as_deref()
        .map(inputs::replacement)
        .transpose()?;
    let overrides = options
        .overrides
        .as_deref()
        .map(inputs::overrides)
        .transpose()?;
    credentials::load(options.alchemy_key_file.as_deref())?;
    let checking = crate::terminal::Progress::start("Checking installed runtimes");
    let (catalog_path, catalog_sha256) = options.runtime.clone().resolve()?;
    let catalog = load_catalog(&catalog_path, &catalog_sha256)?;
    let registry = options.controls.registry()?;
    checking.finish("Installed runtimes verified");
    let data_dir = super::super::super::config::data_directory(options.data_dir.clone())?;
    let cache = if options.no_cache {
        CacheMode::Off
    } else {
        CacheMode::All
    };
    let provider = Arc::new(provider::open(
        &options.controls,
        &data_dir,
        cache,
        controls::remaining(started, &limits)?,
        cancellation.clone(),
    )?);
    let locating = crate::terminal::Progress::start("Locating transaction and historical runtime");
    let mut request = prepare_with_registry(
        signature,
        &provider,
        &catalog,
        replacement,
        overrides,
        limits.clone(),
        &registry,
    )?;
    request.bank_inputs = options.controls.bank_input.clone();
    request.validate()?;
    locating.finish("Historical transaction boundary located");
    let mut inputs: Vec<(Arc<dyn HistoricalSource>, String)> = vec![(provider, "alchemy".into())];
    if let (Some(path), Some(pin)) = (&options.controls.source, &options.controls.source_sha256) {
        let pin = Digest::new(pin.clone())?;
        inputs.push((
            Arc::new(CapturedSource::open(path, &pin).map_err(source_error)?),
            format!("captured:{}", pin.as_str()),
        ));
    }
    let sources = crate::sources::finish_sources(inputs, &data_dir, cache)?
        .ok_or_else(|| Error::new("INTERNAL", "historical provider missing"))?;
    let executable =
        std::env::current_exe().map_err(|e| Error::new("WORKER_OWNER", e.to_string()))?;
    let sha256 = svm_replay_engine::shared::runtime::file_sha256(&executable)
        .map_err(|e| Error::new("WORKER_OWNER", e.to_string()))?;
    let config = Config {
        catalog_path,
        catalog_sha256,
        data_dir,
        owner: ProcessOwner { executable, sha256 },
        cache,
        trace,
    };
    let replaying =
        crate::terminal::Progress::start("Fetching historical state and replaying transaction");
    request.limits.timeout_ms = controls::remaining(started, &limits)?.as_millis() as u64;
    let receipt = simulate_historical(request, &config, sources, cancellation)?;
    replaying.finish("Replay attempt finished");
    Ok(receipt)
}
