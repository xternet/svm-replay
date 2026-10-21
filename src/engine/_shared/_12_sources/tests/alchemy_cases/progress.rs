use super::*;
use svm_replay_engine::shared::progress::{Phase, ProgressJob};

#[test]
fn progress_counts_validated_provider_accounts_and_observed_bytes() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let copy = events.clone();
    let job = ProgressJob::start(
        Phase::FetchingHistoricalAccounts,
        Arc::new(move |event| copy.lock().unwrap().push(event)),
    );
    let source = AlchemySource::with_transport(
        config(),
        limits(),
        Transport::values(vec![genesis(), account()]),
    )
    .unwrap();
    source.inspect(&query(2)).unwrap().unwrap();
    let counters = source.counters().unwrap();
    job.finish("COMPLETED");
    let events = events.lock().unwrap();
    let event = events.last().unwrap();
    assert_eq!(event.fetched_accounts, Some(1));
    assert_eq!(event.downloaded_bytes, Some(counters.downloaded_bytes));
    assert_eq!(event.downloaded_bytes_known, Some(true));
    assert_eq!(event.cache_hits, None);
    assert_eq!(event.processed_earlier_transactions, None);
}

#[test]
fn failed_partial_transports_do_not_fabricate_download_bytes() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let copy = events.clone();
    let job = ProgressJob::start(
        Phase::FetchingHistoricalAccounts,
        Arc::new(move |event| copy.lock().unwrap().push(event)),
    );
    let transport = Transport::values(vec![genesis()]);
    transport
        .responses
        .lock()
        .unwrap()
        .push_back(Err(TransportFailure::Network));
    let source = AlchemySource::with_transport(config(), limits(), transport).unwrap();
    assert!(source.inspect(&query(2)).is_err());
    job.finish("ERROR");
    let events = events.lock().unwrap();
    let event = events.last().unwrap();
    assert_eq!(
        event.downloaded_bytes,
        Some(serde_json::to_vec(&genesis()).unwrap().len() as u64)
    );
    assert_eq!(event.downloaded_bytes_known, Some(false));
    assert_eq!(event.fetched_accounts, Some(0));
}
