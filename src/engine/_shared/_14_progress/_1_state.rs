use super::*;

pub(super) struct State {
    pub phase: Phase,
    pub phase_started: Duration,
    pub advanced: Duration,
    pub emitted: Duration,
    pub sequence: u64,
    pub attempt: u64,
    pub fetched_accounts: Option<u64>,
    pub cache_hits: Option<u64>,
    pub downloaded_bytes: Option<u64>,
    pub downloaded_bytes_known: Option<bool>,
    pub processed_earlier_transactions: Option<u64>,
    pub earlier_transactions_total: Option<Total>,
    pub outcome: Option<String>,
}
impl State {
    pub fn new(phase: Phase) -> Self {
        Self {
            phase,
            phase_started: Duration::ZERO,
            advanced: Duration::ZERO,
            emitted: Duration::ZERO,
            sequence: 0,
            attempt: 0,
            fetched_accounts: None,
            cache_hits: None,
            downloaded_bytes: None,
            downloaded_bytes_known: None,
            processed_earlier_transactions: None,
            earlier_transactions_total: None,
            outcome: None,
        }
    }
    pub fn phase(&mut self, phase: Phase, now: Duration) -> bool {
        if self.phase == phase {
            return false;
        }
        self.phase = phase;
        self.phase_started = now;
        self.advanced = now;
        true
    }
    pub fn total(&mut self, value: u64, final_total: bool, now: Duration) {
        let total = Total {
            value,
            status: if final_total {
                TotalStatus::Final
            } else {
                TotalStatus::Provisional
            },
        };
        if self.earlier_transactions_total != Some(total) {
            self.earlier_transactions_total = Some(total);
            self.advanced = now;
        }
    }
    pub fn event(&mut self, kind: Kind, now: Duration) -> ProgressEvent {
        self.sequence += 1;
        self.emitted = now;
        ProgressEvent {
            schema: "svm-replay-progress/v1",
            origin: "native",
            sequence: self.sequence,
            kind,
            phase: self.phase,
            elapsed_ms: now.as_millis() as u64,
            phase_duration_ms: now.saturating_sub(self.phase_started).as_millis() as u64,
            since_advance_ms: now.saturating_sub(self.advanced).as_millis() as u64,
            attempt: self.attempt,
            fetched_accounts: self.fetched_accounts,
            cache_hits: self.cache_hits,
            downloaded_bytes: self.downloaded_bytes,
            downloaded_bytes_known: self.downloaded_bytes_known,
            processed_earlier_transactions: self.processed_earlier_transactions,
            earlier_transactions_total: self.earlier_transactions_total,
            outcome: self.outcome.clone(),
        }
    }
}
