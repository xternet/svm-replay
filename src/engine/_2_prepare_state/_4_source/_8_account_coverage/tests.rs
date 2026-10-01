use super::*;

#[test]
fn replacement_only_archive_null_requires_independent_absence_evidence() {
    let account = json!({"pubkey":"replacement-only","role":"application",
        "presence":"absent","sourceSlot":19});
    let error = assert_account_coverage(&[account], &[transaction(0)], 19).unwrap_err();
    assert_eq!(error.code, "UNSUPPORTED_HISTORICAL_ACCOUNT");
    assert_eq!(error.details.unwrap()["reason"], "unproven-account-absence");
}

fn transaction(balance: u64) -> Value {
    json!({"transaction":{"message":{"accountKeys":["account"]}},
        "meta":{"preBalances":[balance]}})
}

#[test]
fn null_archive_is_not_proof_of_absence_when_block_shows_funds() {
    let account =
        json!({"pubkey":"account","role":"application","presence":"absent","sourceSlot":19});
    let error = assert_account_coverage(&[account.clone()], &[transaction(7)], 19).unwrap_err();
    assert_eq!(error.code, "UNSUPPORTED_HISTORICAL_ACCOUNT");
    assert_eq!(error.details.unwrap()["historicalPreLamports"], "7");
    assert_account_coverage(&[account.clone()], &[transaction(0), transaction(7)], 19).unwrap();
    let mut present = account.clone();
    present["presence"] = json!("present");
    assert_account_coverage(&[present], &[transaction(7)], 19).unwrap();
    let mut legacy = transaction(0);
    legacy["meta"]["loadedAddresses"] = Value::Null;
    assert_account_coverage(&[account.clone()], &[legacy], 19).unwrap();
    let mut loaded = transaction(7);
    loaded["transaction"]["message"]["accountKeys"] = json!([]);
    loaded["meta"]["loadedAddresses"] = json!({"writable":["account"],"readonly":[]});
    assert!(assert_account_coverage(&[account.clone()], &[loaded], 19).is_err());
    let mut malformed = transaction(7);
    malformed["meta"]["preBalances"] = json!([]);
    assert!(assert_account_coverage(&[account], &[malformed], 19).is_err());
}
