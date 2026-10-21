use super::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn phase_durations_and_stalls_follow_real_advancement() {
    let mut state = State::new(Phase::LocatingTransaction);
    let event = state.event(Kind::Heartbeat, Duration::from_secs(30));
    assert_eq!(event.elapsed_ms, 30000);
    assert_eq!(event.phase_duration_ms, 30000);
    assert_eq!(event.since_advance_ms, 30000);
    state.phase(Phase::ResolvingDependencies, Duration::from_secs(31));
    let event = state.event(Kind::Heartbeat, Duration::from_secs(61));
    assert_eq!(event.phase_duration_ms, 30000);
    assert_eq!(event.since_advance_ms, 30000);
    assert_eq!(event.outcome, None);
}

#[test]
fn totals_can_grow_and_become_final_without_processing_work() {
    let mut state = State::new(Phase::ResolvingDependencies);
    state.total(3, false, Duration::from_secs(1));
    state.total(7, false, Duration::from_secs(2));
    assert_eq!(
        state
            .event(Kind::Advance, Duration::from_secs(2))
            .earlier_transactions_total,
        Some(Total {
            value: 7,
            status: TotalStatus::Provisional
        })
    );
    state.total(7, true, Duration::from_secs(3));
    let event = state.event(Kind::Advance, Duration::from_secs(3));
    assert_eq!(
        event.earlier_transactions_total.unwrap().status,
        TotalStatus::Final
    );
    assert_eq!(event.processed_earlier_transactions, None);
}

#[test]
fn cached_inputs_and_downloads_are_separate_from_execution() {
    let (events, sink) = recorder();
    let job = ProgressJob::start(Phase::LocatingTransaction, sink);
    cache_hit();
    account_fetched();
    downloaded(42);
    earlier_total(9, true);
    job.finish("COMPLETED");
    let events = events.lock().unwrap();
    let event = events.last().unwrap();
    assert_eq!(event.cache_hits, Some(1));
    assert_eq!(event.fetched_accounts, Some(1));
    assert_eq!(event.downloaded_bytes, Some(42));
    assert_eq!(event.processed_earlier_transactions, None);
    assert_eq!(event.outcome.as_deref(), Some("COMPLETED"));
}

#[test]
fn heartbeat_runs_during_blocking_wait_and_stops_on_every_terminal_path() {
    for outcome in [
        "COMPLETED",
        "ERROR",
        "CANCELLED",
        "TIMEOUT",
        "NEEDS_INPUT",
        "UNSUPPORTED",
        "MISMATCH",
    ] {
        let (events, sink) = recorder();
        let job = ProgressJob::start_with_interval(
            Phase::FetchingHistoricalAccounts,
            sink,
            Duration::from_millis(5),
        );
        std::thread::sleep(Duration::from_millis(25));
        job.finish(outcome);
        let before = events.lock().unwrap().len();
        std::thread::sleep(Duration::from_millis(15));
        let events = events.lock().unwrap();
        assert_eq!(events.len(), before);
        assert!(events.iter().any(|e| e.kind == Kind::Heartbeat));
        assert_eq!(
            events.iter().filter(|e| e.kind == Kind::Terminal).count(),
            1
        );
        assert_eq!(events.last().unwrap().outcome.as_deref(), Some(outcome));
        for pair in events.windows(2) {
            assert_eq!(pair[1].sequence, pair[0].sequence + 1);
        }
    }
    assert_eq!(HEARTBEAT_INTERVAL, Duration::from_secs(30));
}

#[test]
fn drop_terminates_and_jobs_do_not_share_counters() {
    let (events, sink) = recorder();
    {
        let _job = ProgressJob::start(Phase::LocatingTransaction, sink);
        cache_hit();
    }
    assert_eq!(
        events.lock().unwrap().last().unwrap().outcome.as_deref(),
        Some("ERROR")
    );
    let (events, sink) = recorder();
    let job = ProgressJob::start(Phase::VerifyingOriginal, sink);
    processed_earlier(0);
    job.finish("COMPLETED");
    let events = events.lock().unwrap();
    assert_eq!(events.last().unwrap().cache_hits, None);
    assert_eq!(
        events.last().unwrap().processed_earlier_transactions,
        Some(0)
    );
}

fn recorder() -> (Arc<Mutex<Vec<ProgressEvent>>>, Sink) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let copy = events.clone();
    (
        events,
        Arc::new(move |event| copy.lock().unwrap().push(event)),
    )
}

#[test]
fn zero_worker_metrics_do_not_hide_a_stall() {
    let (_events, sink) = recorder();
    let job = ProgressJob::start(Phase::ReplayingEarlierTransactions, sink);
    let before = CURRENT.with(|slot| {
        slot.borrow()
            .as_ref()
            .unwrap()
            .state
            .lock()
            .unwrap()
            .advanced
    });
    processed_earlier(0);
    downloaded(0);
    let after = CURRENT.with(|slot| {
        slot.borrow()
            .as_ref()
            .unwrap()
            .state
            .lock()
            .unwrap()
            .advanced
    });
    assert_eq!(before, after);
    job.finish("COMPLETED");
}

#[test]
fn production_timer_emits_after_thirty_seconds_without_advancement() {
    let (sender, receiver) = std::sync::mpsc::channel();
    let job = ProgressJob::start(
        Phase::FetchingHistoricalAccounts,
        Arc::new(move |event| sender.send(event).unwrap()),
    );
    assert_eq!(receiver.recv().unwrap().kind, Kind::Phase);
    let event = receiver
        .recv_timeout(Duration::from_secs(32))
        .expect("30-second heartbeat");
    assert_eq!(event.kind, Kind::Heartbeat);
    assert!(event.elapsed_ms >= 30000);
    assert!(event.elapsed_ms < 32000);
    assert_eq!(event.since_advance_ms, event.elapsed_ms);
    job.finish("TIMEOUT");
    assert_eq!(receiver.recv().unwrap().kind, Kind::Terminal);
    assert!(receiver.try_recv().is_err());
}
