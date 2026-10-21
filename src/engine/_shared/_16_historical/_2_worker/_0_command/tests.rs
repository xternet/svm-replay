use super::*;
use serde_json::json;

#[test]
fn commands_round_trip_with_native_names_and_reject_unknown_fields() {
    for command in [
        json!({"method":"state"}),
        json!({"method":"accounts","params":{"pubkeys":["address"]}}),
        json!({"method":"setBalance","params":{"pubkey":"address","lamports":"18446744073709551615"}}),
        json!({"method":"overrideAccounts","params":{"entries":[{"pubkey":"address","lamports":"123"}]}}),
        json!({"method":"send","params":{"transactionBase64":"wire","sigVerify":true}}),
        json!({"method":"simulate","params":{"transactionBase64":"wire","sigVerify":false}}),
        json!({"method":"configureRpc","params":{"context":{"schema":"svm-native-rpc-context/v1"}}}),
        json!({"method":"rpcSend","params":{"transactionBase64":"wire","skipPreflight":false}}),
        json!({"method":"rpcSimulate","params":{"transactionBase64":"wire","sigVerify":true,"replaceRecentBlockhash":false}}),
        json!({"method":"latestBlockhash"}),
        json!({"method":"signatureStatuses","params":{"signatures":["signature"]}}),
        json!({"method":"rent","params":{"dataLen":165}}),
        json!({"method":"feeForMessage","params":{"messageBase64":"message"}}),
        json!({"method":"blockhashValid","params":{"blockhash":"hash"}}),
        json!({"method":"proveBlockhashQueue","params":{"headers":[{"slot":9,"blockhash":"a",
            "previousBlockhash":"b","parentSlot":8}]}}),
        json!({"method":"select","params":{"forkId":7}}),
        json!({"method":"closeFork","params":{"forkId":7}}),
        json!({"method":"reset"}),
        json!({"method":"close"}),
    ] {
        let decoded: SessionCommand = serde_json::from_value(command.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), command);
    }
    for invalid in [
        json!({"method":"unknown"}),
        json!({"method":"setBalance","params":{"pubkey":"address","lamports":1}}),
        json!({"method":"setBalance","params":{"pubkey":"address","lamports":"1","historical":true}}),
        json!({"method":"state","unexpected":true}),
        json!({"method":"send","params":{"transactionBase64":"wire"}}),
        json!({"method":"select","params":{"forkId":1,"ignored":true}}),
        json!({"method":"rpcSend","params":{"transactionBase64":"wire"}}),
        json!({"method":"rpcSimulate","params":{"transactionBase64":"wire","sigVerify":true}}),
    ] {
        assert!(serde_json::from_value::<SessionCommand>(invalid).is_err());
    }
}
