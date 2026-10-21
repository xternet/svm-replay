use super::*;
use svm_replay_engine::_1_resolve_runtime::InstalledCatalog;
use svm_replay_engine::shared::historical::After;

struct RetainedExecution {
    receipt: Value,
    hydration: Option<svm_replay_engine::shared::historical::SessionHydration>,
    catalog: InstalledCatalog,
}

pub(crate) fn run(options: Options) -> Result<Value, Error> {
    let receipt = match execute(&options) {
        Ok(receipt) => receipt,
        Err(error) => crate::arguments::error_receipt(error),
    };
    // Selector failures are already preserved in receipt; this is a filename label only.
    let after_signature = match options.after.as_deref().map(str::parse::<After>) {
        Some(Ok(After::Transaction(signature))) => Some(signature),
        _ => None,
    };
    output::finish(
        receipt,
        options
            .tx
            .as_deref()
            .or(after_signature.as_deref())
            // silent-ok: filename label only; finish preserves errors (saving_incomplete_output_keeps_category_and_never_adds_result test).
            .unwrap_or(""),
        &options.fields,
        options.out.as_deref(),
    )
}

pub(in super::super) fn execute(options: &Options) -> Result<Value, Error> {
    let cancellation = crate::lifecycle::cancellation::install()?;
    execute_with_cancel(options, cancellation)
}

pub(in super::super) fn execute_with_cancel(
    options: &Options,
    cancellation: svm_replay_engine::shared::runtime::CancellationToken,
) -> Result<Value, Error> {
    Ok(execute_retained(options, cancellation, false)?.receipt)
}

pub(crate) fn execute_for_session(
    options: &Options,
    cancellation: svm_replay_engine::shared::runtime::CancellationToken,
) -> Result<
    (
        Value,
        svm_replay_engine::shared::historical::SessionHydration,
        InstalledCatalog,
    ),
    Error,
> {
    let result = execute_retained(options, cancellation, true)?;
    Ok((
        result.receipt,
        result
            .hydration
            .ok_or_else(|| Error::new("INTERNAL", "session provider lifetime missing"))?,
        result.catalog,
    ))
}

fn execute_retained(
    options: &Options,
    cancellation: svm_replay_engine::shared::runtime::CancellationToken,
    retain: bool,
) -> Result<RetainedExecution, Error> {
    let started = Instant::now();
    let limits = options.controls.limits()?;
    let selector = options
        .after
        .as_deref()
        .map(str::parse::<After>)
        .transpose()?
        .or_else(|| options.tx.clone().map(After::Transaction))
        .ok_or_else(|| Error::new("INVALID_REQUEST", "--tx or --after is required"))?;
    let payload = options.payload.as_deref().map(inputs::bundle).transpose()?;
    if let After::Transaction(signature) = &selector {
        svm_replay_engine::shared::history::address(signature, 64)?;
    }
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
    let bundle = |value: &Option<String>| value.as_deref().map(inputs::bundle).transpose();
    let (replacement, overrides, mut what_if) = inputs::variants(
        bundle(&options.replace)?,
        match payload {
            Some(payload) => Some(payload),
            None => bundle(&options.append)?,
        },
        options
            .overrides
            .as_deref()
            .map(inputs::overrides)
            .transpose()?,
    )?;
    if matches!(&selector, After::Slot(_)) {
        if let Some(variant) = &mut what_if {
            variant.mode = svm_replay_protocol::WhatIfMode::Replace;
        }
    }
    if what_if.is_some() && tracing {
        return Err(Error::new(
            "TRACE_UNSUPPORTED_VARIANT",
            "--collect does not support bundles or state overrides yet",
        ));
    }
    let provider_config = options
        .historical
        .resolve(options.alchemy_key_file.as_deref())?;
    let checking = crate::terminal::Progress::start("Checking installed runtimes");
    let (catalog_path, catalog_sha256) = options.runtime.clone().resolve()?;
    let catalog = load_catalog(&catalog_path, &catalog_sha256)?;
    let registry = options.controls.registry()?;
    checking.finish("Installed runtimes verified");
    let data_dir = crate::arguments::data_directory(options.data_dir.clone())?;
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
        provider_config,
    )?);
    let locating = crate::terminal::Progress::start("Locating transaction and historical runtime");
    let boundary = match selector {
        After::Transaction(signature) => Boundary::Transaction(prepare_with_registry(
            &signature,
            &provider,
            &catalog,
            replacement,
            overrides,
            limits.clone(),
            &registry,
        )?),
        After::Slot(after) => {
            if tracing
                || !options.controls.bank_input.is_empty()
                || replacement.is_some()
                || overrides.is_some()
            {
                return Err(Error::new("UNSUPPORTED_INITIALIZED_OPTION",
                    "slot initialization does not yet accept tracing, external Bank inputs or legacy replacements"));
            }
            Boundary::Initialization(svm_replay_engine::_2_prepare_state::source::signature::prepare_initialization_with_registry(
                after, &provider, &catalog, limits.clone(), &registry)?)
        }
    };
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
    let mut request = match boundary {
        Boundary::Transaction(request) => request,
        Boundary::Initialization(mut request) => {
            request.limits.timeout_ms = controls::remaining(started, &limits)?.as_millis() as u64;
            let (receipt, hydration) =
                _1_initialization::run(request, what_if, &config, sources, cancellation, retain)?;
            replaying.finish("Historical initialization attempt finished");
            return Ok(RetainedExecution {
                receipt,
                hydration,
                catalog,
            });
        }
    };
    request.what_if = what_if;
    request.bank_inputs = options.controls.bank_input.clone();
    request.validate()?;
    request.limits.timeout_ms = controls::remaining(started, &limits)?.as_millis() as u64;
    let hydration = if retain {
        Some(
            svm_replay_engine::shared::historical::SessionHydration::new(
                request.clone(),
                config.clone(),
                sources.fresh_journal(),
            )?,
        )
    } else {
        None
    };
    let receipt = simulate_historical(request, &config, sources, cancellation)?;
    replaying.finish("Replay attempt finished");
    Ok(RetainedExecution {
        receipt,
        hydration,
        catalog,
    })
}
