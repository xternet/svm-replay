use super::*;

/// Optional durable conservative reservations, not exact downloaded-byte totals.
/// Owns a distinct Store handle so cached-source fill cannot reenter its mutex.
pub struct DurableRpcBudget {
    pub(in super::super) store: Mutex<Store>,
    pub(in super::super) id: String,
    pub(in super::super) limits: Value,
}

pub(in super::super) fn counts(limits: &SourceLimits) -> Value {
    json!({"reads":limits.max_account_reads,"requests":limits.max_requests,"bytes":limits.max_download_bytes})
}

impl DurableRpcBudget {
    pub fn open(mut store: Store, id: String, limits: &SourceLimits) -> Result<Self> {
        public_id(
            &json!(id),
            "SOURCE_CONFIGURATION",
            "invalid public budget id",
        )?;
        validate_limits(limits)?;
        let limits = counts(limits);
        store
            .execute(json!({"version":1,"op":"budget","action":"open","id":id,"limits":limits}))
            .map_err(|error| map_error(error, &id))?;
        Ok(Self {
            store: Mutex::new(store),
            id,
            limits,
        })
    }
    pub(in super::super::super) fn matches(&self, limits: &SourceLimits) -> bool {
        self.limits == counts(limits)
    }
    /// `reserved` contains payload bounds plus the reader's one lookahead byte,
    /// including reservations for completed short reads.
    /// Real downloaded bytes are available only on each AlchemySource's counters.
    pub fn snapshot(&self) -> Result<Value> {
        let result = self
            .store
            .lock()
            .map_err(|_| SourceError::new("SOURCE_BUDGET_ERROR", "budget store poisoned"))?
            .execute(json!({"version":1,"op":"budget","action":"inspect","id":self.id}))
            .map_err(|error| map_error(error, &self.id))?;
        Ok(
            json!({"schema":"m17-rpc-budget/v1","id":self.id,"accounting":"full-response-reservation/no-refund","limits":result["limits"],"reserved":result["used"]}),
        )
    }
    /// Atomically commits the complete allowed response bound before transport.
    pub(in super::super::super) fn reserve(&self, account: bool, maximum: u64) -> Result<u64> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| SourceError::new("SOURCE_BUDGET_ERROR", "budget store poisoned"))?;
        let state = store
            .execute(json!({"version":1,"op":"budget","action":"inspect","id":self.id}))
            .map_err(|error| map_error(error, &self.id))?;
        let used = state["used"]["bytes"]
            .as_u64()
            .ok_or_else(|| SourceError::new("SOURCE_BUDGET_ERROR", "invalid stored byte count"))?;
        let limit = self.limits["bytes"]
            .as_u64()
            .ok_or_else(|| SourceError::new("SOURCE_BUDGET_ERROR", "invalid stored byte limit"))?;
        let remaining = limit.checked_sub(used).ok_or_else(|| {
            SourceError::new("SOURCE_RESOURCE_LIMIT", "durable source budget exhausted")
        })?;
        let requested = maximum.checked_add(1).ok_or_else(|| {
            SourceError::new("SOURCE_RESOURCE_LIMIT", "response reservation overflow")
        })?;
        let reserved = requested.min(remaining);
        require(
            reserved > 1,
            "SOURCE_RESOURCE_LIMIT",
            "durable source byte budget cannot cover payload and lookahead",
        )?;
        let bound = reserved - 1;
        store.execute(json!({"version":1,"op":"budget","action":"charge","id":self.id,"amount":{"reads":u64::from(account),"requests":1,"bytes":reserved}}))
            .map_err(|error| map_error(error, &self.id))?;
        // Independent handles/processes may race inspect; Store's atomic charge
        // rejects the loser. No retry or network attempt follows a failed charge.
        Ok(bound)
    }
    pub(in super::super::super) fn observe_excess(&self, bytes: u64) -> Result<()> {
        self.store.lock()
            .map_err(|_| SourceError::new("SOURCE_BUDGET_ERROR", "budget store poisoned"))?
            .execute(json!({"version":1,"op":"budget","action":"observe","id":self.id,"amount":{"reads":0,"requests":0,"bytes":bytes}}))
            .map_err(|error| map_error(error, &self.id))?;
        Ok(())
    }
}

pub(in super::super) fn map_error(error: StoreError, id: &str) -> SourceError {
    let code = match error.code {
        "BUDGET_EXCEEDED" => "SOURCE_RESOURCE_LIMIT",
        "BUDGET_CONFLICT" => "SOURCE_BUDGET_CONFLICT",
        _ => "SOURCE_BUDGET_ERROR",
    };
    SourceError::new(code, "durable historical RPC budget operation failed")
        .details(json!({"budgetId":id,"storeCode":error.code}))
}
