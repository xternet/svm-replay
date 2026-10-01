use super::*;

pub(in super::super) fn bind_evidence(
    evidence: &Value,
    fields: &[&str],
    domain: &str,
) -> Check<Value> {
    shape(evidence, &["expectedGenesisHash", "source", "snapshot"])?;
    pubkey(field(evidence, "expectedGenesisHash")?)?;
    let snapshot = field(evidence, "snapshot")?;
    shape(snapshot, fields)?;
    let source = field(evidence, "source")?;
    let proof = sha256(format!(
        "{domain}\n{}\n",
        canonical_json(&json!({"snapshot":snapshot,"source":source}))
    ));
    let mut bound = snapshot.clone();
    bound["source"] = source.clone();
    bound["proofHash"] = json!(proof);
    Ok(bound)
}

/// Compute a bound complete current-epoch snapshot and validate exact Clock/Bank context.
pub fn bind_epoch_stake_evidence(input: &Value) -> Result<Value, Error> {
    let bind = || -> Check<Value> {
        let evidence = field(input, "evidence")?;
        let bound = bind_evidence(evidence, EPOCH_FIELDS, "svm-bound-epoch-stakes/v1")?;
        let mut fixture = field(input, "fixture")?.clone();
        object(field(&fixture, "runtime")?)?;
        fixture["runtime"]["epochStakes"] = bound.clone();
        epoch_binding(
            &fixture,
            Some(string(field(evidence, "expectedGenesisHash")?)?),
        )?;
        Ok(bound)
    };
    bind().map_err(|e| Error::new("INVALID_EPOCH_STAKE_INPUT", e))
}

/// Apply only explicit caller-trusted pre-transaction stake pairs matching every parent image.
pub fn prepare_exact_initialized_stakes(input: &Value) -> Result<Value, Error> {
    let prepare = || -> Check<Value> {
        let evidence = field(input, "evidence")?;
        let bound = bind_evidence(
            evidence,
            INITIALIZED_FIELDS,
            "svm-bound-initialized-stakes/v1",
        )?;
        let snapshot = initialized_snapshot(&bound)?;
        equal(
            field(&snapshot, "genesisHash")?,
            field(evidence, "expectedGenesisHash")?,
            "genesis mismatch",
        )?;
        for (observed, expected) in [
            ("slot", "slot"),
            ("parentSlot", "parentSlot"),
            ("blockhash", "blockhash"),
            ("blockEvidenceSha256", "blockSourceHash"),
        ] {
            equal(
                field(&snapshot, observed)?,
                field(input, expected)?,
                "canonical context mismatch",
            )?;
        }
        runtime_matches(&snapshot, field(input, "runtime")?)?;
        required_stake_set(field(input, "accounts")?, &snapshot, "parent")?;
        let mut initialized = BTreeMap::new();
        for row in array(field(&snapshot, "accounts")?)? {
            let account = field(row, "initialized")?;
            initialized.insert(string(field(account, "pubkey")?)?, account);
        }
        let accounts = array(field(input, "accounts")?)?
            .iter()
            .map(|account| -> Check<Value> {
                let id = string(field(account, "pubkey")?)?;
                Ok(match initialized.get(id) {
                    Some(value) => (*value).clone(),
                    None => account.clone(),
                })
            })
            .collect::<Check<Vec<_>>>()?;
        Ok(json!({"accounts":accounts,"snapshot":bound}))
    };
    prepare().map_err(|e| Error::new("INVALID_INITIALIZED_STAKE_INPUT", e))
}

/// Match complete current-epoch cache evidence to exact target Clock/Bank/runtime.
pub fn assert_epoch_stake_binding(
    fixture: &Value,
    expected_genesis: Option<&str>,
) -> Result<(), Error> {
    epoch_binding(fixture, expected_genesis)
        .map_err(|error| Error::new("INVALID_EPOCH_STAKE_INPUT", error))
}

/// Match exact post-initialization stake images without deriving rewards or reusing end-state images.
pub fn assert_initialized_stake_binding(
    fixture: &Value,
    expected_genesis: Option<&str>,
) -> Result<(), Error> {
    initialized_binding(fixture, expected_genesis)
        .map_err(|error| Error::new("INVALID_INITIALIZED_STAKE_INPUT", error))
}

/// Control preflight: optional bound inputs are validated, never silently ignored.
pub fn assert_bound_context(fixture: &Value) -> Result<(), Error> {
    assert_epoch_stake_binding(fixture, None)?;
    assert_initialized_stake_binding(fixture, None)?;
    super::super::super::migration::assert_program_migration_bindings(fixture)?;
    let runtime =
        field(fixture, "runtime").map_err(|error| Error::new("INVALID_BOUND_CONTEXT", error))?;
    if runtime.get("epochStakes").is_some() {
        let capabilities = runtime
            .pointer("/binding/executor/m9Build/capabilities")
            .and_then(Value::as_array);
        if !capabilities.is_some_and(|values| {
            values
                .iter()
                .any(|value| value == "guarded-epoch-stakes/v1")
        }) {
            return Err(Error::new(
                "UNSUPPORTED_WORKER_CAPABILITY",
                "guarded-epoch-stakes/v1 required for supplied epoch context",
            ));
        }
    }
    Ok(())
}
