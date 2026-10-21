use super::*;

impl SessionHydration {
    /// The canonical request's account scope as every expansion so far widened it.
    pub(in super::super::super) fn extra_accounts(&self) -> Vec<String> {
        match &self.request {
            CanonicalRequest::Transaction(request) => request.extra_accounts.clone(),
            CanonicalRequest::Initialization(request) => request.extra_accounts.clone(),
        }
    }
    /// Puts back a recorded expansion's account scope: a restored fork expands later reads
    /// from the same scope as the session that recorded it.
    pub(in super::super::super) fn restore_extra_accounts(&mut self, accounts: Vec<String>) {
        match &mut self.request {
            CanonicalRequest::Transaction(request) => request.extra_accounts = accounts,
            CanonicalRequest::Initialization(request) => request.extra_accounts = accounts,
        }
    }
    /// The retained receipt (path, SHA-256) the last expansion's Hydrate binds.
    pub(in super::super::super) fn last_receipt(&self) -> Option<&(std::path::PathBuf, String)> {
        self.last_receipt.as_ref()
    }
}
