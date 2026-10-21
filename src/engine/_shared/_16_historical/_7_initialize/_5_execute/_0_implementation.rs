use super::*;

pub fn execute(
    mut input: InitializationRequest,
    payload: WhatIf,
    config: &Config,
    sources: CompositeSource,
    budget: &ExecutionBudget,
) -> Result<Value, Error> {
    payload.validate()?;
    if !payload.state_overrides.is_empty()
        && !input.runtime_binding["executor"]["m9Build"]["capabilities"]
            .as_array()
            .is_some_and(|caps| caps.iter().any(|cap| cap == "native-account-overrides/v1"))
    {
        return Err(Error::new(
            "CAPABILITY_UNAVAILABLE",
            "installed worker does not support initialized account overrides",
        ));
    }
    for entry in &payload.state_overrides {
        if !input.extra_accounts.contains(&entry.pubkey) {
            input.extra_accounts.push(entry.pubkey.clone());
        }
    }
    let transactions = payload
        .transactions_base64
        .as_ref()
        .ok_or_else(|| invalid("initialized-slot execution requires a transaction payload"))?;
    for wire in transactions {
        let decoded = svm_replay_protocol::transaction::decode(wire)?;
        let message = &decoded["transaction"]["message"];
        for key in message["accountKeys"]
            .as_array()
            .ok_or_else(|| invalid("payload keys missing"))?
        {
            let key = key
                .as_str()
                .ok_or_else(|| invalid("payload address malformed"))?;
            if !input.extra_accounts.iter().any(|id| id == key) {
                input.extra_accounts.push(key.into());
            }
        }
        for lookup in message["addressTableLookups"]
            .as_array()
            .ok_or_else(|| invalid("payload lookup tables missing"))?
        {
            let key = lookup["accountKey"]
                .as_str()
                .ok_or_else(|| invalid("payload lookup address missing"))?;
            if !input.extra_accounts.iter().any(|id| id == key) {
                input.extra_accounts.push(key.into());
            }
        }
    }
    // A complete replacement or explicit deletion defines the requested state
    // without claiming an unavailable historical before-image.
    input.extra_accounts.retain(|key| {
        !payload.state_overrides.iter().any(|entry| {
            &entry.pubkey == key
                && (entry.owner.is_some() || entry.lamports.as_deref() == Some("0"))
        })
    });
    let hydrated =
        SessionHydration::initialized(input.clone(), config.clone(), sources.fresh_journal())?;
    let canonical = initialize(input.clone(), config, sources, budget)?;
    if canonical["outcome"] != "COMPLETED" {
        return Ok(canonical);
    }
    let path: PathBuf = canonical["receiptPath"]
        .as_str()
        .ok_or_else(|| invalid("initialization receipt path missing"))?
        .into();
    let catalog = load_catalog(&config.catalog_path, &config.catalog_sha256)?;
    let worker = catalog
        .manifest
        .workers
        .iter()
        .find(|w| w.family == input.family)
        .ok_or_else(|| invalid("initialized worker missing from catalog"))?;
    let descriptor = SessionInput {
        schema: "svm-historical-session-input/v1".into(),
        receipt_sha256: file_sha256(&path).map_err(protocol_error)?,
        receipt_path: path,
        worker_path: catalog.root.join(&worker.file),
        after: false,
    };
    let started = Instant::now();
    let request =
        json!({"requestId":input.request_id,"initialization":descriptor,"payload":payload});
    let (root, mut receipt, _coordinator) =
        job::begin_job(&request, config, "initialized-slot-payload/v1")?;
    receipt["worker"] = canonical["worker"].clone();
    receipt["runtime"] = canonical["runtime"].clone();
    if let Some(selection) = canonical.get("runtimeSelection") {
        receipt["runtimeSelection"] = selection.clone();
    }
    receipt["verification"] = canonical["verification"].clone();
    receipt["originalControl"] = canonical["originalControl"].clone();
    receipt["reconstructedFromSources"] = json!(true);
    let result = (|| {
        let hash = write_json(&root.join("payload-request.json"), &request)?;
        receipt["payloadRequest"] = json!({"file":"payload-request.json","sha256":hash});
        let mut session = VerifiedSession::open(
            descriptor,
            WorkerTransport::new(config.data_dir.join("scratch")).with_owner(config.owner.clone()),
            WorkerLimits::default(),
            budget.clone(),
        )?
        .with_hydration(hydrated)?;
        let pins = session.pins().clone();
        let states = if payload.state_overrides.is_empty() {
            json!([])
        } else {
            let changed = session.request(
                SessionCommand::OverrideAccounts {
                    entries: payload.state_overrides.clone(),
                },
                budget,
            )?;
            changed
                .get("stateOverrides")
                .filter(|value| value.is_array())
                .ok_or_else(|| invalid("native state override evidence missing"))?
                .clone()
        };
        let mut outputs = Vec::new();
        for wire in transactions {
            let output = session.request(
                SessionCommand::Send {
                    transaction_base64: wire.clone(),
                    sig_verify: false,
                },
                budget,
            )?;
            if !output["status"].is_string() || !output["logs"].is_array() {
                return Err(
                    invalid("native payload execution output malformed").with_details(output)
                );
            }
            outputs.push(output);
        }
        let state = session.request(SessionCommand::State, budget)?;
        session.close()?;
        Ok(
            json!({"schema":"svm-initialized-slot-payload-output/v1","original":null,
            "originalControl":canonical["originalControl"],"pins":pins,"state":state,
            "whatIf":{"mode":"replace","stateOverrides":states,"transactions":outputs}}),
        )
    })();
    job::finish_job(result, receipt, &root, started)
}
