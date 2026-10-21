use super::*;

fn update(change: impl FnOnce(&mut State, Duration) -> Option<Kind>) {
    CURRENT.with(|slot| {
        if let Some(job) = slot.borrow().as_ref() {
            job.update(change);
        }
    });
}
fn advancement(state: &mut State, now: Duration) -> Option<Kind> {
    state.advanced = now;
    // Counters can advance rapidly. Phase changes, totals and terminal are immediate.
    (now.saturating_sub(state.emitted) >= Duration::from_secs(1)).then_some(Kind::Advance)
}
pub fn phase(phase: Phase) {
    update(|state, now| state.phase(phase, now).then_some(Kind::Phase));
}
pub fn transport_started() {
    update(|state, _| {
        state.fetched_accounts.get_or_insert(0);
        state.downloaded_bytes.get_or_insert(0);
        state.downloaded_bytes_known.get_or_insert(true);
        Some(Kind::Advance)
    });
}
pub fn account_fetched() {
    update(|state, now| {
        *state.fetched_accounts.get_or_insert(0) += 1;
        advancement(state, now)
    });
}
pub fn cache_hit() {
    update(|state, now| {
        *state.cache_hits.get_or_insert(0) += 1;
        advancement(state, now)
    });
}
pub fn downloaded(bytes: u64) {
    update(|state, now| {
        *state.downloaded_bytes.get_or_insert(0) += bytes;
        if bytes > 0 {
            advancement(state, now)
        } else {
            None
        }
    });
}
pub fn download_unknown() {
    update(|state, _| {
        state.downloaded_bytes_known = Some(false);
        Some(Kind::Advance)
    });
}
pub fn earlier_total(value: u64, final_total: bool) {
    update(|state, now| {
        state.total(value, final_total, now);
        Some(Kind::Advance)
    });
}
pub fn attempt(prefix_count: u64) {
    update(|state, now| {
        state.attempt += 1;
        state.processed_earlier_transactions = None;
        state.total(prefix_count, true, now);
        Some(Kind::Advance)
    });
}
pub fn processed_earlier(count: u64) {
    update(|state, now| {
        *state.processed_earlier_transactions.get_or_insert(0) += count;
        if count > 0 {
            advancement(state, now)
        } else {
            None
        }
    });
}
