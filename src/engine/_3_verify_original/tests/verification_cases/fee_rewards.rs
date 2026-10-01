use super::*;

#[test]
fn end_slot_fee_reward_is_not_a_transaction_write() {
    let (candidate, mut fixture, mut block, output) = context();
    let balance = fixture["endAccounts"][0]["lamports"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    fixture["endAccounts"][0]["lamports"] = json!((balance + 17).to_string());
    block["result"]["rewards"] = json!([{"pubkey":"payer","rewardType":"Fee",
        "lamports":17,"postBalance":balance + 17,"commission":null}]);
    let result = verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
    )
    .unwrap();
    assert_eq!(result.0["endSlotFeeRewards"][0]["lamports"], "17");
    for invalid in [json!(balance + 18), json!(0)] {
        let mut bad = block.clone();
        bad["result"]["rewards"][0]["postBalance"] = invalid;
        assert!(verify(&candidate, &fixture, &bad, &output, MetadataPolicy::Strict).is_err());
    }
    for amount in [json!(-17), json!(u64::MAX)] {
        let mut bad = block.clone();
        bad["result"]["rewards"][0]["lamports"] = amount;
        assert!(verify(&candidate, &fixture, &bad, &output, MetadataPolicy::Strict).is_err());
    }
    let mut bad = block.clone();
    bad["result"]["rewards"][0]["rewardType"] = json!("Staking");
    assert!(verify(&candidate, &fixture, &bad, &output, MetadataPolicy::Strict).is_err());
    let reward = block["result"]["rewards"][0].clone();
    block["result"]["rewards"]
        .as_array_mut()
        .unwrap()
        .push(reward);
    assert!(verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict
    )
    .is_err());
}
