use super::*;

#[test]
fn branches_stay_isolated_when_handles_interleave() {
    let (mut forks, mut branches) = (Forks::new(), Branches::new(vec!["a".to_string()]));
    let copy = branches.fork(&mut forks, 0, &budget()).unwrap();
    branches
        .run(&mut forks, copy, send("credit:a:5"), &budget())
        .unwrap();
    assert_eq!(lamports(&mut forks, &mut branches, 0, "a"), json!("10"));
    branches
        .run(&mut forks, 0, send("credit:a:1"), &budget())
        .unwrap();
    assert_eq!(lamports(&mut forks, &mut branches, copy, "a"), json!("15"));
    assert_eq!(lamports(&mut forks, &mut branches, 0, "a"), json!("11"));
    // Each switch selects once; repeated requests on one branch select nothing.
    assert_eq!(forks.selects, vec![copy, 0, copy, 0]);
}

#[test]
fn fork_management_stays_with_the_backend_and_the_last_branch_stays_open() {
    let (mut forks, mut branches) = (Forks::new(), Branches::new(vec!["a".to_string()]));
    for command in [
        SessionCommand::Snapshot,
        SessionCommand::Select { fork_id: 0 },
        SessionCommand::CloseFork { fork_id: 0 },
        SessionCommand::Close,
        // A blockhash proof is the session's own, read from its sources.
        SessionCommand::ProveBlockhashQueue { headers: json!([]) },
    ] {
        let error = branches.run(&mut forks, 0, command, &budget()).unwrap_err();
        assert_eq!(error.code, "INVALID_BACKEND_COMMAND");
    }
    let copy = branches.fork(&mut forks, 0, &budget()).unwrap();
    branches.enter(&mut forks, copy, &budget()).unwrap();
    // Closing the active branch selects another first, as the worker requires.
    branches.close(&mut forks, copy, &budget()).unwrap();
    assert_eq!(forks.active, 0);
    assert_eq!(
        branches.close(&mut forks, 0, &budget()).unwrap_err().code,
        "LAST_BRANCH"
    );
    let unknown = branches
        .run(&mut forks, copy, send("x"), &budget())
        .unwrap_err();
    assert_eq!(unknown.code, "UNKNOWN_BRANCH");
}

#[test]
fn a_fork_commits_into_its_parent_only_while_the_parent_is_unchanged() {
    let (mut forks, mut branches) = (Forks::new(), Branches::new(vec!["a".to_string()]));
    let sandbox = branches.fork(&mut forks, 0, &budget()).unwrap();
    branches
        .run(&mut forks, sandbox, send("credit:a:4"), &budget())
        .unwrap();
    assert_eq!(branches.revision(sandbox).unwrap(), 1);
    // Reads do not count as changes.
    lamports(&mut forks, &mut branches, 0, "a");
    branches.commit(&mut forks, sandbox, 0, &budget()).unwrap();
    assert_eq!(
        lamports(&mut forks, &mut branches, sandbox, "a"),
        json!("14")
    );
    assert_eq!(branches.revision(0).unwrap_err().code, "UNKNOWN_BRANCH");
    assert_eq!(
        forks.forks.keys().copied().collect::<Vec<_>>(),
        vec![sandbox]
    );

    let stale = branches.fork(&mut forks, sandbox, &budget()).unwrap();
    branches
        .run(&mut forks, sandbox, send("credit:a:1"), &budget())
        .unwrap();
    let error = branches
        .commit(&mut forks, stale, sandbox, &budget())
        .unwrap_err();
    assert_eq!(error.code, "STALE_BRANCH");
    assert_eq!(
        lamports(&mut forks, &mut branches, sandbox, "a"),
        json!("15")
    );
    let error = branches
        .commit(&mut forks, sandbox, stale, &budget())
        .unwrap_err();
    assert_eq!(error.code, "INVALID_COMMIT");
}

#[test]
fn a_restore_puts_boundary_images_back_without_touching_other_accounts() {
    let (mut forks, mut branches) = (Forks::new(), Branches::new(vec!["a".to_string()]));
    branches
        .run(&mut forks, 0, send("credit:a:5"), &budget())
        .unwrap();
    branches
        .run(&mut forks, 0, send("credit:b:2"), &budget())
        .unwrap();
    let reply = _4_restore::restore(
        &mut branches,
        &mut forks,
        0,
        vec!["a".into(), "b".into()],
        &budget(),
    )
    .unwrap();
    assert_eq!(reply["restored"][1], Value::Null);
    assert_eq!(lamports(&mut forks, &mut branches, 0, "a"), json!("10"));
    assert_eq!(lamports(&mut forks, &mut branches, 0, "b"), Value::Null);
    assert_eq!(forks.forks.len(), 1, "the boundary copy is discarded");
    assert!(_4_restore::restore(&mut branches, &mut forks, 0, vec![], &budget()).is_err());
}

#[test]
fn the_inventory_lists_what_each_fork_loaded_and_resets_with_it() {
    let (mut forks, mut branches) = (Forks::new(), Branches::new(vec!["a".to_string()]));
    let balance = |key: &str| SessionCommand::SetBalance {
        pubkey: key.into(),
        lamports: "3".into(),
    };
    let copy = branches.fork(&mut forks, 0, &budget()).unwrap();
    branches
        .run(&mut forks, copy, balance("b"), &budget())
        .unwrap();
    forks.hydrate = vec!["h".into()];
    branches
        .run(&mut forks, copy, send("credit:a:1"), &budget())
        .unwrap();
    let keys = |branches: &Branches, branch| branches.inventory.keys(branch).unwrap();
    assert_eq!(keys(&branches, copy), vec!["a", "b", "h"]);
    assert_eq!(keys(&branches, 0), vec!["a"]);
    let known = _5_inventory::known_accounts(&mut branches, &mut forks, copy, &budget()).unwrap();
    assert_eq!(
        known[1],
        json!({"pubkey": "b", "account": {"pubkey": "b", "lamports": "3"}})
    );
    branches
        .run(&mut forks, copy, SessionCommand::Reset, &budget())
        .unwrap();
    assert_eq!(keys(&branches, copy), vec!["a"]);
    branches.close(&mut forks, copy, &budget()).unwrap();
    assert!(branches.inventory.keys(copy).is_err());
    // A key the inventory lists but the fork has not loaded is an integrity failure.
    assert_eq!(
        branches
            .inventory
            .add(copy, ["x".to_string()])
            .unwrap_err()
            .code,
        "SESSION_INTEGRITY"
    );
    branches.inventory.add(0, ["ghost".to_string()]).unwrap();
    let error = _5_inventory::known_accounts(&mut branches, &mut forks, 0, &budget()).unwrap_err();
    assert_eq!(error.code, "SESSION_INTEGRITY");
}
