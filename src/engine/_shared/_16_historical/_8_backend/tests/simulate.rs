use super::*;

#[test]
fn simulation_post_bytes_come_from_a_discarded_execution_that_reproduced_it() {
    let (mut forks, mut branches) = (Forks::new(), Branches::new(vec!["a".to_string()]));
    let reply = _2_simulate::simulate(
        &mut branches,
        &mut forks,
        0,
        simulation("credit:a:7", &["a", "b"]),
        &budget(),
    )
    .unwrap();
    assert_eq!(reply["postAccounts"][0]["lamports"], json!("17"));
    assert_eq!(reply["postAccounts"][1], Value::Null);
    assert_eq!(reply["simulation"]["output"]["computeUnits"], json!(150));
    // The base branch is unchanged and the copy is gone.
    assert_eq!(lamports(&mut forks, &mut branches, 0, "a"), json!("10"));
    assert_eq!(forks.forks.len(), 1);

    let failed = _2_simulate::simulate(
        &mut branches,
        &mut forks,
        0,
        simulation("fail", &["a"]),
        &budget(),
    )
    .unwrap();
    assert_eq!(failed["postAccounts"], Value::Null);
    assert_eq!(forks.forks.len(), 1);

    forks.diverge = true;
    let error = _2_simulate::simulate(
        &mut branches,
        &mut forks,
        0,
        simulation("credit:a:7", &["a"]),
        &budget(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SIMULATION_DIVERGED");
    assert_eq!(forks.forks.len(), 1, "a diverged copy is still discarded");
    assert_eq!(forks.active, 0);
}

#[test]
fn a_failed_cleanup_stays_visible_beside_the_failure_that_caused_it() {
    let (mut forks, mut branches) = (Forks::new(), Branches::new(vec!["a".to_string()]));
    forks.diverge = true;
    forks.fail_close = true;
    let error = _2_simulate::simulate(
        &mut branches,
        &mut forks,
        0,
        simulation("credit:a:7", &["a"]),
        &budget(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SIMULATION_DIVERGED");
    let details = error.details.unwrap();
    assert_eq!(details["field"], json!("computeUnits"));
    assert_eq!(details["cleanupError"]["code"], json!("WORKER_EXITED"));
}
