use super::*;

pub(in super::super) struct State {
    pub(in super::super) counters: SourceCounters,
    pub(in super::super) genesis_raw: Option<Vec<u8>>,
    pub(in super::super) inspected: bool,
}

pub struct AlchemySource {
    pub(in super::super) discovery_cache: Option<Mutex<svm_replay_store::Store>>,
    pub(in super::super) cancellation: Option<crate::shared::runtime::CancellationToken>,
    pub(in super::super) identity: Value,
    pub(in super::super) limits: SourceLimits,
    pub(in super::super) expires: Instant,
    pub(in super::super) transport: Arc<dyn RpcTransport>,
    pub(in super::super) state: Mutex<State>,
    pub(in super::super) budget: Option<Arc<DurableRpcBudget>>,
}
impl AlchemySource {
    pub fn from_environment(config: AlchemyConfig, limits: SourceLimits) -> Result<Self> {
        Self::with_transport(
            config,
            limits,
            Arc::new(HttpsTransport::from_environment()?),
        )
    }
}
impl AlchemySource {
    pub fn with_transport(
        config: AlchemyConfig,
        limits: SourceLimits,
        transport: Arc<dyn RpcTransport>,
    ) -> Result<Self> {
        let identity = json!({"id":config.id,"version":config.version,"kind":"alchemy-historical","genesisHash":config.expected_genesis_hash,"coverage":{"firstSlot":config.first_slot,"lastSlot":config.last_slot,"completeness":"partial"},"capabilities":["account","block","transaction"]});
        validate_identity(&identity)?;
        validate_limits(&limits)?;
        let expires = Instant::now()
            .checked_add(limits.deadline)
            .ok_or_else(|| SourceError::new("SOURCE_CONFIGURATION", "invalid source deadline"))?;
        Ok(Self {
            discovery_cache: None,
            cancellation: None,
            identity,
            limits,
            expires,
            transport,
            state: Mutex::new(State {
                counters: SourceCounters::default(),
                genesis_raw: None,
                inspected: false,
            }),
            budget: None,
        })
    }
}
impl AlchemySource {
    /// Cooperative cancellation checks before and after each bounded HTTP request.
    pub fn with_cancellation(mut self, token: crate::shared::runtime::CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }
}
impl AlchemySource {
    pub(in super::super) fn check_cancelled(&self) -> Result<()> {
        if self
            .cancellation
            .as_ref()
            .is_some_and(|token| token.is_cancelled())
        {
            return Err(SourceError::new(
                "CANCELLED",
                "historical acquisition cancelled",
            ));
        }
        Ok(())
    }
}
impl AlchemySource {
    /// Attach only before inspection. Budget IDs can deliberately span providers.
    pub fn with_budget(mut self, budget: Arc<DurableRpcBudget>) -> Result<Self> {
        require(
            self.budget.is_none()
                && !self
                    .state
                    .lock()
                    .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "source state poisoned"))?
                    .inspected
                && budget.matches(&self.limits),
            "SOURCE_CONFIGURATION",
            "budget requires an unused source and matching limits",
        )?;
        self.budget = Some(budget);
        Ok(self)
    }
}
impl AlchemySource {
    pub fn counters(&self) -> Result<SourceCounters> {
        Ok(self
            .state
            .lock()
            .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "source state poisoned"))?
            .counters
            .clone())
    }
}
impl AlchemySource {
    /// Acquire a previously unpinned block at an explicit reviewed slot. The
    /// returned standard record pins the bytes and retains their RPC provenance.
    /// This is provider evidence, not an independent consensus proof.
    pub(in super::super) fn discover_block_uncached(&self, target_slot: u64) -> Result<Value> {
        let result = (|| {
            let mut state = self
                .state
                .lock()
                .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "source state poisoned"))?;
            state.inspected = true;
            slot(&json!(target_slot), "INVALID_QUERY", "target slot")?;
            let coverage = &self.identity["coverage"];
            require(
                target_slot >= slot(&coverage["firstSlot"], "SOURCE_INTEGRITY", "first slot")?
                    && target_slot <= slot(&coverage["lastSlot"], "SOURCE_INTEGRITY", "last slot")?,
                "SOURCE_UNAVAILABLE",
                "target slot outside configured coverage",
            )?;
            if state.genesis_raw.is_none() {
                let raw = self.post(
                    &mut state,
                    &json!({"jsonrpc":"2.0","id":1,"method":"getGenesisHash","params":[]}),
                    false,
                )?;
                checked_genesis(&raw, &self.identity["genesisHash"])?;
                state.genesis_raw = Some(raw);
            }
            let mut query = json!({"kind":"block","genesisHash":self.identity["genesisHash"],"slot":target_slot});
            let request = request_for_query(&query)?;
            let raw = self.post(&mut state, &request, false)?;
            query["blockEvidenceSha256"] = json!(Digest::of(&raw));
            let genesis = state.genesis_raw.as_ref().ok_or_else(|| {
                SourceError::new("SOURCE_INTEGRITY", "genesis verification state missing")
            })?;
            record_from_response(&query, &request, &raw, genesis)
        })();
        if result.is_err() {
            self.state
                .lock()
                .map_err(|_| {
                    SourceError::new(
                        "SOURCE_INTEGRITY",
                        "source state poisoned while recording failure",
                    )
                })?
                .counters
                .failed_inspections += 1;
        }
        result
    }
}
impl AlchemySource {
    pub(in super::super) fn remaining(&self) -> Result<Duration> {
        self.expires
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| {
                SourceError::new("SOURCE_DEADLINE", "historical source deadline exceeded")
            })
    }
}
