use super::*;

pub fn initialize(
    input: InitializationRequest,
    config: &Config,
    mut sources: CompositeSource,
    budget: &ExecutionBudget,
) -> Result<Value, Error> {
    input.validate()?;
    sources
        .assert_genesis(&input.genesis_hash)
        .map_err(history::source_error)?;
    if config.trace.is_some() {
        return Err(invalid("initialized sessions do not accept replay tracing"));
    }
    let catalog = load_catalog(&config.catalog_path, &config.catalog_sha256)?;
    let worker = catalog
        .manifest
        .workers
        .iter()
        .find(|w| w.family == input.family)
        .ok_or_else(|| Error::new("WORKER_IDENTITY", "initialized runtime is not installed"))?;
    let executor = &input.runtime_binding["executor"];
    if executor["id"] != worker.executor_source_id
        || executor["m9Build"]["binarySha256"] != worker.sha256.as_str()
        || executor["m9Build"]["buildHash"] != worker.build_hash.as_str()
        || executor["sourceEvidenceHash"] != worker.source_sha256.as_str()
        || !worker
            .capabilities
            .iter()
            .any(|cap| cap == "native-initialized-slot/v1")
    {
        return Err(Error::new(
            "WORKER_IDENTITY",
            "initialized runtime capability/identity differs",
        ));
    }
    let started = Instant::now();
    let (root, mut receipt, _coordinator) = job::begin_job(
        &serde_json::to_value(&input).map_err(|e| invalid(e.to_string()))?,
        config,
        "source-reconstructed-slot-initialization/v1",
    )?;
    let result = (|| {
        let mut history = History::new(
            &mut sources,
            input.genesis_hash.clone(),
            input.slot,
            budget,
            10000,
        )?;
        let packet = prepare(&input, &mut history)?;
        validate(&packet)?;
        let pin = write_json(&root.join("initialized-request.json"), &packet)?;
        receipt["initializedRequest"] = json!({"file":"initialized-request.json","sha256":pin});
        receipt["reconstructedFromSources"] = json!(true);
        receipt["worker"] = json!({"family":worker.family,"executorSourceId":worker.executor_source_id,
            "sha256":worker.sha256,"buildHash":worker.build_hash,"sourceSha256":worker.source_sha256});
        receipt["runtime"] = json!({"agave":executor["agaveFeatureSetVersion"],"profileId":input.runtime_binding["runtimeProfileId"]});
        receipt["originalControl"] = packet["originalControl"].clone();
        let scratch = config.data_dir.join("scratch");
        _5_finalize::ensure_private_directory(&scratch)?;
        let native = NativeSession::open(
            WorkerTransport::new(&scratch).with_owner(config.owner.clone()),
            WorkerSpec {
                executable: catalog.root.join(&worker.file),
                sha256: worker.sha256.as_str().into(),
            },
            packet["fixture"].clone(),
            false,
            WorkerLimits::default(),
            budget.clone(),
        )?;
        let ready = native.ready().clone();
        native.close()?;
        receipt["verification"] = json!({"status":"INITIALIZED","runtimeBindingHash":input.runtime_binding["bindingHash"]});
        Ok(
            json!({"schema":"svm-native-initialization-result/v1","original":null,
            "originalControl":packet["originalControl"],"ready":ready}),
        )
    })();
    receipt["sourceDiagnostics"] = match sources.diagnostics() {
        Ok(value) => value,
        Err(error) => json!({"status":"ERROR","error":error}),
    };
    let result = match write_json(
        &root.join("source-observations.json"),
        &json!(sources.observations()),
    ) {
        Ok(hash) => {
            receipt["sourceObservations"] =
                json!({"file":"source-observations.json","sha256":hash});
            result.map_err(|error| crate::shared::sources::attribute_failure(error, &sources))
        }
        Err(error) => Err(error.with_details(json!({"primaryError":result.err()}))),
    };
    job::finish_job(result, receipt, Path::new(&root), started)
}
