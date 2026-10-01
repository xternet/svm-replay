use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use svm_replay_engine::shared::bank::imports::{dedicated_inputs, inspect};
fn program(code: &str) -> serde_json::Value {
    json!({"pubkey":"program","presence":"present","executable":true,"owner":"native",
      "dataBase64":STANDARD.encode(code.as_bytes())})
}
#[test]
fn dedicated_dependencies_are_distinct_from_generic_runtime_reads() {
    let accounts = vec![program(
        "sol_get_clock_sysvar\0sol_get_epoch_rewards_sysvar\0sol_get_sysvar",
    )];
    let programs = vec!["program".to_owned()];
    assert_eq!(
        dedicated_inputs(&programs, &accounts).unwrap(),
        vec!["SysvarEpochRewards1111111111111111111111111"]
    );
    assert!(inspect(&programs, &accounts, &[], false).is_err());
    assert!(inspect(
        &programs,
        &accounts,
        &["SysvarEpochRewards1111111111111111111111111".into()],
        false
    )
    .is_ok());
}
#[test]
fn fees_and_missing_epoch_snapshot_do_not_use_defaults() {
    let programs = vec!["program".to_owned()];
    assert!(inspect(&programs, &[program("sol_get_fees_sysvar")], &[], true).is_err());
    assert!(inspect(&programs, &[program("sol_get_epoch_stake")], &[], false).is_err());
    assert!(inspect(&programs, &[program("sol_get_epoch_stake")], &[], true).is_ok());
}
