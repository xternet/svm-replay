use super::*;

/// Owns canonical read scope and the original provider instances/budget. Secrets
/// stay in those providers, never in native commands or provenance artifacts.
pub struct SessionHydration {
    pub(in super::super) request: CanonicalRequest,
    pub(in super::super) config: Config,
    pub(in super::super) sources: CompositeSource,
    /// The retained receipt (path, SHA-256) of the last expansion, which its Hydrate binds.
    pub(in super::super) last_receipt: Option<(std::path::PathBuf, String)>,
}
impl SessionHydration {
    pub fn new(
        request: HistoricalRequest,
        config: Config,
        sources: CompositeSource,
    ) -> Result<Self, Error> {
        request.validate()?;
        if request.what_if.is_some()
            || request.replacement_transaction_base64.is_some()
            || request.requested_account_overrides.is_some()
        {
            return Err(Error::new(
                "INVALID_SESSION_INPUT",
                "hydration requires canonical unmodified history",
            ));
        }
        Ok(Self {
            request: CanonicalRequest::Transaction(request),
            config,
            sources,
            last_receipt: None,
        })
    }
    pub fn initialized(
        request: InitializationRequest,
        config: Config,
        sources: CompositeSource,
    ) -> Result<Self, Error> {
        request.validate()?;
        Ok(Self {
            request: CanonicalRequest::Initialization(request),
            config,
            sources,
            last_receipt: None,
        })
    }
    pub(in super::super::super) fn assert_anchor(&self, pins: &Value) -> Result<(), Error> {
        let after = pins["after"]
            .as_bool()
            .ok_or_else(|| Error::new("HYDRATION_CONFLICT", "pinned boundary phase missing"))?;
        let index = pins["transactionIndex"]
            .as_u64()
            .and_then(|i| i.checked_sub(u64::from(after)))
            .ok_or_else(|| Error::new("HYDRATION_CONFLICT", "pinned boundary index missing"))?;
        let valid = match &self.request {
            CanonicalRequest::Transaction(request) => {
                request.genesis_hash == pins["genesisHash"]
                    && request.after_slot == pins["afterSlot"].as_u64()
                    && request.family == pins["family"]
                    && request.candidate["slot"] == pins["slot"]
                    && request.candidate["transactionIndex"] == index
                    && request.runtime_binding["bindingHash"] == pins["runtimeBindingHash"]
            }
            CanonicalRequest::Initialization(request) => {
                !after
                    && index == 0
                    && pins["boundaryKind"] == "initialized-slot"
                    && request.genesis_hash == pins["genesisHash"]
                    && request.family == pins["family"]
                    && request.slot == pins["slot"]
                    && pins["afterSlot"].as_u64() == Some(request.after_slot)
                    && request.runtime_binding["bindingHash"] == pins["runtimeBindingHash"]
            }
        };
        if !valid {
            return Err(Error::new(
                "HYDRATION_CONFLICT",
                "provider/read scope differs from historical session",
            ));
        }
        Ok(())
    }
}
