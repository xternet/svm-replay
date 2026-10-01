use super::*;

pub(in super::super) fn execute(command: Command) -> Result<Value, Error> {
    match command {
        Command::Demo {
            example,
            bundle,
            bundle_sha256,
            data_dir,
        } => {
            if example.is_none()
                && crate::terminal::enabled()
                && crate::terminal::interactive()
                && crate::terminal::select(
                    "Choose an offline example",
                    &["Bundled historical replay (cached inputs, no API key)"],
                )?
                .is_none()
            {
                return Ok(json!({"outcome":"CANCELLED"}));
            }
            crate::demo::run(bundle, bundle_sha256, config::data_directory(data_dir)?)
        }
        Command::Validate { request } => crate::request::inspect(&request),
        Command::Request {
            candidate,
            block,
            signature,
            alchemy,
            source_output,
            data_dir,
            runtime_binding,
            family,
            genesis_hash,
            request_id,
            output,
            replacement_base64,
            overrides,
            bank_input,
            timeout_ms,
            max_output_bytes,
            max_diagnostic_bytes,
        } => {
            let binding = crate::request::read(&runtime_binding)?;
            svm_replay_protocol::runtime::validate_runtime_binding(&binding)?;
            let block = if let Some(path) = alchemy {
                let slot = binding["targetSlot"]
                    .as_u64()
                    .ok_or_else(|| Error::new("RUNTIME_BINDING", "target slot missing"))?;
                let source = crate::sources::alchemy_source(
                    &path,
                    &config::data_directory(data_dir)?,
                    timeout_ms,
                )?;
                use svm_replay_engine::shared::sources::HistoricalSource;
                if source.identity()["genesisHash"] != genesis_hash {
                    return Err(Error::new(
                        "SOURCE_CONTEXT_MISMATCH",
                        "provider and requested genesis differ",
                    ));
                }
                let record = source
                    .discover_block(slot)
                    .map_err(svm_replay_engine::shared::history::source_error)?;
                let raw = svm_replay_engine::shared::history::data(
                    record["value"]["rawBase64"]
                        .as_str()
                        .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "raw block missing"))?,
                )?;
                let path = source_output
                    .ok_or_else(|| Error::new("INVALID_REQUEST", "source output required"))?;
                crate::request::save(
                    &path,
                    &json!({"source":source.identity(),"record":record,"diagnostics":source.diagnostics().map_err(svm_replay_engine::shared::history::source_error)?}),
                )?;
                Some(raw)
            } else {
                block
                    .map(|path| {
                        read_bounded_file(&path, 256 * 1024 * 1024)
                            .map_err(|e| Error::new("REQUEST_IO", e.to_string()))
                    })
                    .transpose()?
            };
            let (candidate, archived) = match (candidate, block, signature) {
                (Some(path), None, None) => (crate::request::read(&path)?, None),
                (None, Some(raw), Some(signature)) => (
                    crate::request::boundary(&raw, &signature, &binding)?,
                    Some(parse_json(&raw)?),
                ),
                _ => {
                    return Err(Error::new(
                        "INVALID_REQUEST",
                        "supply candidate or block and signature",
                    ))
                }
            };
            crate::request::build(
                candidate,
                archived,
                binding,
                family,
                genesis_hash,
                request_id,
                replacement_base64,
                overrides.map(|p| crate::request::read(&p)).transpose()?,
                bank_input,
                Limits {
                    timeout_ms,
                    max_output_bytes,
                    max_diagnostic_bytes,
                },
                &output,
            )
        }
        Command::Bundle { command } => match command {
            BundleCommand::Pack {
                catalog,
                catalog_sha256,
                output,
                family,
                reference_only,
            } => crate::bundle::pack(
                &catalog,
                &Digest::new(catalog_sha256)?,
                &output,
                &family,
                reference_only,
            ),
            BundleCommand::Install {
                bundle,
                bundle_sha256,
                output,
            } => crate::bundle::install(&bundle, &Digest::new(bundle_sha256)?, &output),
        },
        Command::Capabilities { runtime } | Command::Doctor { runtime } => {
            let (catalog, catalog_sha256) = runtime.resolve()?;
            let installed =
                svm_replay_engine::_1_resolve_runtime::load_catalog(&catalog, &catalog_sha256)?;
            Ok(
                json!({"outcome":"COMPLETED","schema":"svm-replay-capabilities/v1","workers":installed.manifest.workers,
                "platform":installed.manifest.platform,"captureWorkers":installed.manifest.capture_workers,"preparedReplay":true,"sourceReconstruction":true,"releaseQualified":false}),
            )
        }
        Command::Simulate {
            request,
            runtime,
            data_dir,
            cache,
            source,
            source_sha256,
            trace,
            debug,
            alchemy,
            genesis_hash,
        } => {
            let (catalog, catalog_sha256) = runtime.resolve()?;
            let cancellation = crate::lifecycle::cancellation::install()?;
            let bytes = read_bounded_file(&request, 256 * 1024 * 1024)
                .map_err(|e| Error::new("REQUEST_IO", e.to_string()))?;
            let schema = parse_json(&bytes)?["schema"]
                .as_str()
                .ok_or_else(|| Error::new("INVALID_REQUEST", "request schema required"))?
                .to_owned();
            let executable =
                std::env::current_exe().map_err(|e| Error::new("WORKER_OWNER", e.to_string()))?;
            let sha256 =
                file_sha256(&executable).map_err(|e| Error::new("WORKER_OWNER", e.to_string()))?;
            let trace = trace
                .map(|path| {
                    let bytes = read_bounded_file(&path, 1024 * 1024)
                        .map_err(|e| Error::new("TRACE_OPTIONS_IO", e.to_string()))?;
                    let options: TraceOptions = serde_json::from_value(parse_json(&bytes)?)
                        .map_err(|e| Error::new("TRACE_OPTIONS", e.to_string()))?;
                    options.bounds.validate(&options.capture)?;
                    Ok::<_, Error>(options)
                })
                .transpose()?;
            let config = Config {
                catalog_path: catalog,
                catalog_sha256,
                data_dir: config::data_directory(data_dir)?,
                owner: ProcessOwner { executable, sha256 },
                cache: match cache {
                    CacheChoice::Off => CacheMode::Off,
                    CacheChoice::Prepared => CacheMode::Prepared,
                    CacheChoice::All => CacheMode::All,
                },
                trace,
            };
            match schema.as_str() {
                "svm-replay-prepared/v1" => {
                    let request = PreparedRequest::parse(&bytes)?;
                    if debug {
                        if source.is_some() || alchemy.is_some() || genesis_hash.is_some() {
                            return Err(Error::new("DEBUG_CONFIG", "hydrate through simulate first; live debugging never retries user commands"));
                        }
                        return crate::debugger::run(request, &config, cancellation);
                    }
                    let sources = crate::sources::configured(
                        captured_pair(source, source_sha256)?,
                        alchemy,
                        &config.data_dir,
                        config.cache,
                        request.limits.timeout_ms,
                    )?;
                    match sources {
                        Some(sources) => simulate_prepared_with_sources(
                            request,
                            &config,
                            sources,
                            genesis_hash.ok_or_else(|| {
                                Error::new(
                                    "INVALID_REQUEST",
                                    "prepared source hydration requires --genesis-hash",
                                )
                            })?,
                            cancellation,
                        ),
                        None => {
                            if genesis_hash.is_some() {
                                return Err(Error::new(
                                    "INVALID_REQUEST",
                                    "--genesis-hash has no source to bind",
                                ));
                            }
                            simulate_prepared(request, &config, cancellation)
                        }
                    }
                }
                "svm-replay-historical/v1" => {
                    if debug {
                        return Err(Error::new("DEBUG_CONFIG", "simulate historical sources first, then debug the saved prepared-request.json"));
                    }
                    if genesis_hash.is_some() {
                        return Err(Error::new("INVALID_REQUEST","historical requests bind genesisHash in the request, not a second flag"));
                    }
                    let request = HistoricalRequest::parse(&bytes)?;
                    let sources=crate::sources::configured(captured_pair(source,source_sha256)?,alchemy,&config.data_dir,config.cache,request.limits.timeout_ms)?
                        .ok_or_else(||Error::new("INVALID_REQUEST","historical request needs an explicit --source or --alchemy configuration"))?;
                    simulate_historical(request, &config, sources, cancellation)
                }
                _ => Err(Error::new("INVALID_REQUEST", "unknown request schema")),
            }
        }
    }
}
