use super::*;
use std::sync::{Arc, Mutex};
use svm_replay_engine::shared::progress::{Phase, ProgressJob, TotalStatus};

#[test]
fn dependency_discovery_emits_growing_provisional_prefix_totals() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let copy = events.clone();
    let job = ProgressJob::start(
        Phase::ResolvingDependencies,
        Arc::new(move |event| copy.lock().unwrap().push(event)),
    );
    let transactions = [
        semantic(0, &["program-data"], &["program-data"], "a"),
        semantic(1, &["state"], &["state"], "b"),
        semantic(2, &[], &["state", "program"], "c"),
    ];
    let closure = compute_program_data_fixed_point(&transactions, 2, |keys| {
        Ok(keys
            .iter()
            .map(|key| {
                (
                    key.clone(),
                    if key == "program" {
                        Some("program-data".into())
                    } else {
                        None
                    },
                )
            })
            .collect())
    })
    .unwrap();
    assert_eq!(closure.selected_indices, vec![0, 1]);
    job.finish("COMPLETED");
    let events = events.lock().unwrap();
    let totals = events
        .iter()
        .filter_map(|e| e.earlier_transactions_total)
        .collect::<Vec<_>>();
    assert!(totals
        .iter()
        .any(|t| t.value == 1 && t.status == TotalStatus::Provisional));
    assert!(totals
        .iter()
        .any(|t| t.value == 2 && t.status == TotalStatus::Provisional));
    assert!(events
        .iter()
        .all(|e| e.processed_earlier_transactions.is_none()));
}
