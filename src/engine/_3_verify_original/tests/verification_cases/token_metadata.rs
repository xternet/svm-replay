use super::*;

#[test]
fn token_metadata_is_not_collected_without_a_token_program() {
    let (candidate, fixture, mut block, output) = context();
    let meta = &mut block["result"]["transactions"][0]["meta"];
    meta["preTokenBalances"] = json!([]);
    meta["postTokenBalances"] = json!([]);
    let result = verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
    )
    .unwrap();
    assert_eq!(result.0["original"]["immediateTokenAmounts"], 0);
    for key in [
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
    ] {
        let mut recorded = block.clone();
        recorded["result"]["transactions"][0]["transaction"]["message"]["accountKeys"][1] =
            json!(key);
        assert!(verify(
            &candidate,
            &fixture,
            &recorded,
            &output,
            MetadataPolicy::Strict
        )
        .is_err());
    }
    let mut wrong = output.clone();
    wrong["original"]["accountTransitions"][0]["before"]["lamports"] = json!("1");
    assert!(verify(&candidate, &fixture, &block, &wrong, MetadataPolicy::Strict).is_err());
    let mut wrong = output.clone();
    wrong["original"]["endAccountStates"][0]["state"]["dataHash"] = json!(hash(b"wrong"));
    assert!(verify(&candidate, &fixture, &block, &wrong, MetadataPolicy::Strict).is_err());
    let (candidate, fixture, block, mut output) = context();
    output["original"]["accountTransitions"][0]["before"]["tokenAmount"] = json!("8");
    assert!(verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict
    )
    .is_err());
}
