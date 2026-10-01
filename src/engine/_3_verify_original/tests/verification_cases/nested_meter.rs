use super::*;

#[test]
fn rejected_meter_difference_retains_observed_values() {
    let archived = "Program Jupiter consumed 81349 of 133177 compute units";
    let replay = "Program Jupiter consumed 81334 of 133193 compute units";
    let error = compare_archived_compute_metadata(
        "prefix",
        &[archived.into()],
        &[replay.into()],
        129515,
        129485,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains(archived));
    assert!(error.contains(replay));
    assert!(error.contains("total delta 30"));
}

#[test]
fn rejected_meter_difference_retains_later_changed_lines() {
    let archive = vec![
        "Program A consumed 12 of 100 compute units".into(),
        "Program B consumed 13 of 88 compute units".into(),
    ];
    let replay = vec![
        "Program A consumed 14 of 100 compute units".into(),
        "Program B consumed 11 of 90 compute units".into(),
    ];
    let error = compare_archived_compute_metadata("two", &archive, &replay, 25, 21)
        .unwrap_err()
        .to_string();
    assert!(error.contains(&archive[1]));
    assert!(error.contains(&replay[1]));
}

#[test]
fn sequential_meter_costs_accumulate_to_transaction_total() {
    for first in 1..5 {
        for second in 1..5 {
            let archive = vec![
                format!("Program A consumed {} of 100 compute units", 10 + first),
                format!(
                    "Program B consumed {} of {} compute units",
                    10 + second,
                    90 - first
                ),
            ];
            let replay = vec![
                "Program A consumed 10 of 100 compute units".into(),
                "Program B consumed 10 of 90 compute units".into(),
            ];
            let total = 20 + first + second;
            let result =
                compare_archived_compute_metadata("sequential", &archive, &replay, total, 20)
                    .unwrap()
                    .unwrap();
            assert_eq!(result.compute_unit_delta, (first + second) as i64);
            assert!(
                compare_archived_compute_metadata("reverse", &replay, &archive, 20, total).is_ok()
            );
            assert!(compare_archived_compute_metadata(
                "unexplained",
                &archive,
                &replay,
                total + 1,
                20
            )
            .is_err());
            let mut reversed_archive = archive.clone();
            reversed_archive.reverse();
            let mut reversed_replay = replay.clone();
            reversed_replay.reverse();
            assert!(compare_archived_compute_metadata(
                "backwards",
                &reversed_archive,
                &reversed_replay,
                total,
                20
            )
            .is_err());
        }
    }
}

#[test]
fn meter_difference_cannot_silently_disappear() {
    let archive: Vec<String> = vec![
        "Program A consumed 12 of 100 compute units".into(),
        "Program B consumed 10 of 88 compute units".into(),
    ];
    let replay = vec![
        "Program A consumed 10 of 100 compute units".into(),
        archive[1].clone(),
    ];
    assert!(compare_archived_compute_metadata("reset", &archive, &replay, 22, 20).is_err());
}

#[test]
fn nested_meter_divergence_combines_prior_and_local_cost() {
    let replay = vec!["Program Jupiter consumed 81334 of 133193 compute units".into()];
    let archive = vec!["Program Jupiter consumed 81349 of 133178 compute units".into()];
    let report = compare_archived_compute_metadata("nested", &archive, &replay, 129515, 129485)
        .unwrap()
        .unwrap();
    assert_eq!(report.compute_unit_delta, 30);
    assert_eq!(report.different_log_indices, vec![0]);
    assert!(
        compare_archived_compute_metadata("reverse", &replay, &archive, 129485, 129515).is_ok()
    );
    for wrong in [
        "Program Jupiter consumed 81349 of 133177 compute units",
        "Program Jupiter consumed 81365 of 133194 compute units",
        "Program Other consumed 81349 of 133178 compute units",
        "Program log: Jupiter consumed 81349 of 133178 compute units",
    ] {
        assert!(
            compare_archived_compute_metadata("bad", &[wrong.into()], &replay, 129515, 129485)
                .is_err()
        );
    }
}
