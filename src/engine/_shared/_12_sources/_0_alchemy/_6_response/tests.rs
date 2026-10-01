use super::*;

#[test]
fn block_wire_cannot_be_mislabeled_as_transaction_endpoint() {
    let mut bytes = vec![1];
    bytes.extend([2u8; 64]);
    bytes.extend([1, 0, 0, 1]);
    bytes.extend([3u8; 64]);
    bytes.push(0);
    let signature = bs58::encode([2u8; 64]).into_string();
    let genesis = bs58::encode([1u8; 32]).into_string();
    let query = json!({"kind":"transaction","genesisHash":genesis,"slot":99,"signature":signature});
    let block = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"result":{
        "parentSlot":98,"transactions":[{"transaction":[STANDARD.encode(bytes),"base64"]}]}}))
    .unwrap();
    let genesis_raw =
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"result":genesis})).unwrap();
    let request = request_for_query(&query).unwrap();
    assert!(record_from_response(&query, &request, &block, &genesis_raw).is_err());
}

#[test]
fn unsupported_transaction_version_is_not_generic_provider_failure() {
    let raw = json!({"jsonrpc":"2.0","id":1,"error":{"code":-32015,
        "message":"Transaction version is not supported"}});
    let error = envelope(&raw, 1).unwrap_err();
    assert_eq!(error.code, "UNSUPPORTED_TRANSACTION_VERSION");
    assert_eq!(error.details.unwrap()["rpcCode"], -32015);
    for (code, expected) in [(429, "SOURCE_RATE_LIMIT"), (-32000, "SOURCE_RPC_ERROR")] {
        let mut other = raw.clone();
        other["error"]["code"] = json!(code);
        assert_eq!(envelope(&other, 1).unwrap_err().code, expected);
    }
}

#[test]
fn new_block_requests_include_freeze_rewards_and_old_captures_remain_valid() {
    let genesis = bs58::encode([1; 32]).into_string();
    let raw = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"result":{
        "parentSlot":19,"blockhash":genesis,"transactions":[],"rewards":[]}}))
    .unwrap();
    let query = json!({"kind":"block","genesisHash":genesis,"slot":20,
        "blockEvidenceSha256":Digest::of(&raw)});
    let genesis_raw =
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"result":genesis})).unwrap();
    let request = request_for_query(&query).unwrap();
    assert_eq!(request["params"][1]["rewards"], true);
    assert_eq!(request["params"][1]["maxSupportedTransactionVersion"], 1);
    for version in [0, 1] {
        for enabled in [true, false] {
            let mut captured_request = request.clone();
            captured_request["params"][1]["rewards"] = json!(enabled);
            captured_request["params"][1]["maxSupportedTransactionVersion"] = json!(version);
            record_from_response(&query, &captured_request, &raw, &genesis_raw).unwrap();
        }
    }
    let mut unsupported = request.clone();
    unsupported["params"][1]["maxSupportedTransactionVersion"] = json!(2);
    assert!(record_from_response(&query, &unsupported, &raw, &genesis_raw).is_err());
    let mut altered = request;
    altered["params"][1]["commitment"] = json!("processed");
    assert!(record_from_response(&query, &altered, &raw, &genesis_raw).is_err());
}
