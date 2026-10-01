use super::*;

const EXECUTOR: &str = "litesvm-v0.14.0-pr402-agave-4.1.2";

fn accounts(active: u8) -> Vec<Value> {
    let mut data = vec![0u8; 81];
    data[..8].copy_from_slice(&100u64.to_le_bytes());
    data[8..16].copy_from_slice(&3u64.to_le_bytes());
    data[80] = active;
    vec![json!({"pubkey":stake::EPOCH_REWARDS,"sourceSlot":200,
        "dataBase64":STANDARD.encode(data)})]
}

#[test]
fn completed_reward_window_cannot_be_active() {
    let accounts = accounts(1);
    let before = accounts.clone();
    let error = validate(&accounts, EXECUTOR, 200, &json!({"blockHeight":103})).unwrap_err();
    assert_eq!(error.code, "UNSUPPORTED_BANK_INPUT");
    assert_eq!(
        error.details.unwrap()["reason"],
        "inconsistent-epoch-rewards"
    );
    assert_eq!(accounts, before);
}

#[test]
fn final_partition_itself_must_already_be_inactive() {
    assert_eq!(
        validate(&accounts(1), EXECUTOR, 200, &json!({"blockHeight":102}))
            .unwrap_err()
            .code,
        "UNSUPPORTED_BANK_INPUT"
    );
}

#[test]
fn newer_runtime_reports_the_same_proven_inconsistency() {
    let error = validate(
        &accounts(1),
        "litesvm-v0.16.0-agave-4.2.1",
        200,
        &json!({"blockHeight":103}),
    )
    .unwrap_err();
    assert_eq!(error.code, "UNSUPPORTED_BANK_INPUT");
    assert_eq!(
        error.details.unwrap()["reason"],
        "inconsistent-epoch-rewards"
    );
}

#[test]
fn consistency_check_is_not_a_reconstruction_or_runtime_guess() {
    validate(&accounts(1), EXECUTOR, 200, &json!({"blockHeight":101})).unwrap();
    validate(&accounts(0), EXECUTOR, 200, &json!({"blockHeight":103})).unwrap();
    validate(&accounts(1), "unreviewed", 200, &json!({"blockHeight":103})).unwrap();
    validate(&[], EXECUTOR, 200, &json!({})).unwrap();
    assert!(validate(&accounts(1), EXECUTOR, 200, &json!({})).is_err());
    assert!(validate(&accounts(2), EXECUTOR, 200, &json!({"blockHeight":103})).is_err());
}
