use super::_1_capability::{required_capability, TX_V1};
use super::*;

impl VerifiedSession {
    pub fn open(
        input: SessionInput,
        transport: WorkerTransport,
        limits: WorkerLimits,
        budget: ExecutionBudget,
    ) -> Result<Self, Error> {
        let prepared = _2_load::load(&input, &budget)?;
        let base_keys = fixture_keys(&prepared.fixture)?;
        let mut native = NativeSession::open(
            transport,
            prepared.worker,
            prepared.fixture,
            input.after,
            limits,
            budget.clone(),
        )?;
        if native.ready()["original"] != prepared.original {
            return Err(Error::new(
                "SESSION_CONTROL_MISMATCH",
                "fresh native control differs from the pinned verified result",
            ));
        }
        if prepared
            .initialized_state_sha256
            .as_ref()
            .is_some_and(|expected| native.ready()["state"]["stateSha256"] != *expected)
        {
            return Err(Error::new(
                "SESSION_INITIALIZATION_MISMATCH",
                "fresh native initialized state differs from the retained measurement",
            ));
        }
        let state = native.request(
            SessionCommand::ConfigureRpc {
                context: prepared.rpc_context,
            },
            &budget,
        )?;
        check(
            state["slot"] == prepared.pins["slot"]
                && state["transactionIndex"] == prepared.pins["transactionIndex"],
            "fresh native session position differs from source evidence",
        )?;
        Digest::new(text(&state["stateSha256"])?)?;
        let mut pins = prepared.pins;
        pins["initialStateSha256"] = state["stateSha256"].clone();
        let mut ready = native.ready().clone();
        ready["state"] = state;
        Ok(Self {
            input,
            hydration: None,
            native,
            pins,
            ready,
            observer: None,
            base_keys,
            hydrated_keys: Vec::new(),
            boundary: std::sync::Arc::new(prepared.boundary),
            side_log: None,
            side_replay: Default::default(),
            replaying: false,
            queue_headers: Vec::new(),
        })
    }

    pub fn pins(&self) -> &Value {
        &self.pins
    }
    /// The keys the boundary fork holds (see `fixture_keys`).
    pub fn base_keys(&self) -> &[String] {
        &self.base_keys
    }
    /// The keys hydration loaded into the active fork since the last call.
    pub fn take_hydrated_keys(&mut self) -> Vec<String> {
        std::mem::take(&mut self.hydrated_keys)
    }
    pub fn check(&mut self) -> Result<(), Error> {
        self.native.check()
    }
    pub fn ready(&self) -> &Value {
        &self.ready
    }
    pub fn request(
        &mut self,
        command: SessionCommand,
        budget: &ExecutionBudget,
    ) -> Result<Value, Error> {
        let required = required_capability(&command)?;
        if let Some((capability, feature)) = required.filter(|(cap, _)| !self.capable(cap)) {
            return Err(Error::new(
                if capability == TX_V1 {
                    "UNSUPPORTED_TRANSACTION_VERSION"
                } else {
                    "CAPABILITY_UNAVAILABLE"
                },
                format!("installed worker does not support {feature} ({capability})"),
            ));
        }
        let mut attempted = std::collections::BTreeSet::new();
        let mut proven = std::collections::BTreeSet::new();
        loop {
            let error = match self.native.request(command.clone(), budget) {
                Ok(value) => {
                    if let Some(event) =
                        super::super::_6_events::transaction_event(&command, &value)
                    {
                        if let Some(observer) = &mut self.observer {
                            observer(&event);
                        }
                    }
                    return Ok(value);
                }
                Err(error) => error,
            };
            let keys = match &error.details {
                Some(detail)
                    if error.code == "NEEDS_INPUT" && detail["cause"]["kind"] == "account" =>
                {
                    detail["cause"]["pubkeys"]
                        .as_array()
                        .ok_or_else(|| {
                            Error::new("SESSION_PROTOCOL", "missing account list absent")
                        })?
                        .iter()
                        .map(|key| {
                            key.as_str().map(str::to_owned).ok_or_else(|| {
                                Error::new("SESSION_PROTOCOL", "invalid missing account")
                            })
                        })
                        .collect::<Result<Vec<_>, Error>>()?
                }
                // An archived recent blockhash: prove its boundary age, then try again.
                Some(detail)
                    if error.code == "NEEDS_INPUT"
                        && detail["cause"]["kind"] == "recentBlockhash"
                        && self.hydration.is_some()
                        && self.capable(AGES) =>
                {
                    let blockhash = detail["cause"]["blockhash"]
                        .as_str()
                        .ok_or_else(|| Error::new("SESSION_PROTOCOL", "unproven blockhash absent"))?
                        .to_owned();
                    if !proven.insert(blockhash.clone()) {
                        return Err(Error::new(
                            "BLOCKHASH_AGE_NO_PROGRESS",
                            "the same blockhash age remains unproven",
                        )
                        .with_details(json!({"blockhash": blockhash})));
                    }
                    let (proof, headers) = self.next_queue_proof(&blockhash)?;
                    self.native.request(proof, budget)?;
                    self.queue_headers = headers;
                    continue;
                }
                _ => return Err(error),
            };
            if self.hydration.is_none() {
                return Err(error);
            }
            if keys.is_empty() || keys.iter().all(|key| attempted.contains(key)) {
                return Err(Error::new(
                    "HYDRATION_NO_PROGRESS",
                    "same historical inputs remain missing",
                )
                .with_details(serde_json::json!({"pubkeys":keys})));
            }
            attempted.extend(keys.iter().cloned());
            let state = self.native.request(SessionCommand::State, budget)?;
            // A failed acquisition names the inputs it was for, beside its own details.
            let expansion = self
                .next_hydration(&keys, &state, budget)
                .map_err(|mut error| {
                    let keys = serde_json::json!(keys);
                    match &mut error.details {
                        Some(Value::Object(details)) => {
                            details.insert("hydrationKeys".into(), keys);
                        }
                        details => {
                            let previous = details.take();
                            *details = Some(json!({"hydrationKeys": keys, "details": previous}));
                        }
                    }
                    error
                })?;
            let added = match &expansion {
                SessionCommand::Hydrate { fixture, .. } => fixture_keys(fixture)?,
                _ => return Err(Error::new("SESSION_PROTOCOL", "hydration is not a Hydrate")),
            };
            self.native.request(expansion, budget)?;
            self.hydrated_keys.extend(added);
        }
    }
    pub fn with_hydration(mut self, hydration: SessionHydration) -> Result<Self, Error> {
        hydration.assert_anchor(&self.pins)?;
        self.hydration = Some(hydration);
        Ok(self)
    }
    pub fn close(self) -> Result<WorkerOutput, Error> {
        self.native.close()
    }
}
