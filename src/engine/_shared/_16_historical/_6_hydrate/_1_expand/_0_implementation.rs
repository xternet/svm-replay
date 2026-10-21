use super::*;

impl SessionHydration {
    pub(in super::super::super) fn expand(
        &mut self,
        keys: &[String],
        input: &SessionInput,
        pins: &Value,
        state: &Value,
        budget: &ExecutionBudget,
    ) -> Result<SessionCommand, Error> {
        self.assert_anchor(pins)?;
        let mut request = self.request.clone();
        let (extra_accounts, limits) = match &mut request {
            CanonicalRequest::Transaction(value) => (&mut value.extra_accounts, &mut value.limits),
            CanonicalRequest::Initialization(value) => {
                (&mut value.extra_accounts, &mut value.limits)
            }
        };
        for key in keys {
            crate::shared::history::address(key, 32)?;
            if !extra_accounts.contains(key) {
                extra_accounts.push(key.clone());
            }
        }
        extra_accounts.sort();
        // One hydration replay is bounded like any replay request (at most an hour), even
        // inside a longer-lived embedded session.
        let remaining = budget
            .remaining()
            .map_err(protocol_error)?
            .min(std::time::Duration::from_secs(3600));
        limits.timeout_ms = u64::try_from(remaining.as_millis())
            .map_err(|_| Error::new("TIMEOUT", "historical call deadline overflow"))?;
        let (receipt, verification) = match &request {
            CanonicalRequest::Transaction(value) => (
                crate::simulate_historical(
                    value.clone(),
                    &self.config,
                    self.sources.fresh_journal(),
                    budget.cancellation_token(),
                )?,
                "PASS",
            ),
            CanonicalRequest::Initialization(value) => (
                super::super::super::initialize(
                    value.clone(),
                    &self.config,
                    self.sources.fresh_journal(),
                    budget,
                )?,
                "INITIALIZED",
            ),
        };
        if receipt["outcome"] != "COMPLETED" || receipt["verification"]["status"] != verification {
            let code = receipt["error"]["code"].as_str().ok_or_else(|| {
                Error::new(
                    "HYDRATION_UNVERIFIED",
                    "historical expansion did not verify",
                )
            })?;
            let message = receipt["error"]["message"].as_str().ok_or_else(|| {
                Error::new(
                    "HYDRATION_UNVERIFIED",
                    "historical expansion failure lacks context",
                )
            })?;
            // A fork never assumes absence; accounts a test creates are declared instead.
            let hint = if receipt["error"]["details"]["reason"] == "unproven-account-absence" {
                ". If your test creates this account, declare it first: svmReplay_overrideAccounts [{\"pubkey\":\"<address>\",\"lamports\":\"0\"}] (SDK: fork.overrideAccounts)"
            } else {
                ""
            };
            return Err(Error::new(code, format!("{message}{hint}")).with_details(receipt));
        }
        let mut expanded = input.clone();
        expanded.receipt_path = receipt["receiptPath"]
            .as_str()
            .ok_or_else(|| {
                Error::new("HYDRATION_UNVERIFIED", "retained expansion receipt missing")
            })?
            .into();
        expanded.receipt_sha256 = file_sha256(&expanded.receipt_path).map_err(protocol_error)?;
        let prepared = _3_proof::load(&expanded, budget)?;
        for field in [
            "genesisHash",
            "family",
            "signature",
            "slot",
            "transactionIndex",
            "runtimeBindingHash",
            "worker",
            "executionBlockhash",
            "blockhash",
            "afterSlot",
        ] {
            if prepared.pins[field] != pins[field] {
                return Err(Error::new(
                    "HYDRATION_CONFLICT",
                    format!("expanded historical {field} changed"),
                ));
            }
        }
        let state_sha256 = state["stateSha256"]
            .as_str()
            .ok_or_else(|| Error::new("SESSION_PROTOCOL", "active state hash missing"))?
            .to_owned();
        self.request = request;
        self.last_receipt = Some((
            expanded.receipt_path.clone(),
            expanded.receipt_sha256.clone(),
        ));
        Ok(SessionCommand::Hydrate {
            fixture: prepared.fixture,
            original: prepared.original,
            state_sha256,
            receipt_sha256: expanded.receipt_sha256,
            after: input.after,
        })
    }
}
