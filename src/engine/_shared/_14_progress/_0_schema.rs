use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    LocatingTransaction,
    ResolvingDependencies,
    FetchingHistoricalAccounts,
    ReplayingEarlierTransactions,
    VerifyingOriginal,
    SavingInputs,
    ExecutingRequested,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Phase,
    Advance,
    Heartbeat,
    Terminal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TotalStatus {
    Provisional,
    Final,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Total {
    pub value: u64,
    pub status: TotalStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub schema: &'static str,
    pub origin: &'static str,
    pub sequence: u64,
    pub kind: Kind,
    pub phase: Phase,
    pub elapsed_ms: u64,
    pub phase_duration_ms: u64,
    pub since_advance_ms: u64,
    pub attempt: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_accounts: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hits: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloaded_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloaded_bytes_known: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_earlier_transactions: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earlier_transactions_total: Option<Total>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}
