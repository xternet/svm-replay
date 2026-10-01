use super::*;

pub(super) fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

pub(super) fn context() -> (Value, Value, Value, Value) {
    let block_hash = hash(b"synthetic raw source identity");
    let candidate = json!({"id":"unit", "slot":20, "transactionIndex":0,
        "targetSignature":"signature", "blockSourceHash":block_hash,
        "rawEvidenceHash":hash(format!("{block_hash}\n0\nsignature\n")),
        "runtimeProfileId":"profile", "executorSourceId":"worker"});
    let state = json!({"presence":"present", "lamports":"9007199254740993",
        "owner":"program", "executable":false, "rentEpoch":"18446744073709551615",
        "dataHash":hash([1,2,3]), "tokenAmount":"7"});
    let mut after = state.clone();
    after["lamports"] = json!("9007199254740988");
    let fixture = json!({"schema":"svm-simulate-m6-fixture/v1", "caseId":"unit",
        "target":{"targetSlot":20,"parentSlot":19,"index":0,"signature":"signature",
            "prefixIndices":[],"prefixSignatures":[], "transactionBase64":"AQ==",
            "replacementTransactionBase64":null,"evidenceAccounts":["payer"],"omittedFeeEffects":[]},
        "runtime":{"binding":{"targetSlot":20,"runtimeProfileId":"profile",
            "executor":{"id":"worker"},"bindingHash":"binding"}},
        "accountOverride":null,
        "endAccounts":[{"pubkey":"payer","sourceSlot":20,"presence":"present",
            "lamports":"9007199254740988","owner":"program","executable":false,
            "rentEpoch":"18446744073709551615","dataBase64":"AQID"}]});
    let block = json!({"result":{"parentSlot":19,"blockhash":"block",
        "transactions":[{"transaction":{"signatures":["signature"],"message":{
            "header":{"numRequiredSignatures":1,"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":1},
            "accountKeys":["payer","program"],"instructions":[{"programIdIndex":1,"accounts":[0],"data":""}]}},
            "meta":{"err":null,"innerInstructions":[],"logMessages":["Program demo consumed 100 of 200 compute units"],
                "computeUnitsConsumed":100,"fee":5,"preBalances":[9007199254740993u64,0],
                "postBalances":[9007199254740988u64,0],
                "preTokenBalances":[{"accountIndex":0,"uiTokenAmount":{"amount":"7"}}],
                "postTokenBalances":[{"accountIndex":0,"uiTokenAmount":{"amount":"7"}}]}}]}});
    let output = json!({"schema":"svm-simulate-m6-executor-output/v1", "caseId":"unit",
        "executorSourceId":"worker","runtimeBindingHash":"binding","targetIndex":0,"targetSignature":"signature",
        "prefix":[],"omittedFeeEffects":[],"replacement":null,"accountOverride":null,
        "original":{"status":"ok","normalizedError":null,
            "logs":["Program demo consumed 100 of 200 compute units"],"computeUnits":100,"fee":"5","returnData":null,
            "accountTransitions":[{"pubkey":"payer","before":state,"after":after}],
            "endAccountStates":[{"pubkey":"payer","state":after}]}});
    (candidate, fixture, block, output)
}

pub(super) fn trace_context() -> (Value, Value) {
    let bytes = [2, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0];
    let instructions = json!([
        {"path":[1,0],"stackHeight":2,"programIdIndex":3,"accounts":[0,1,1],"dataBase64":STANDARD.encode(bytes)},
        {"path":[1,0,0],"stackHeight":3,"programIdIndex":4,"accounts":[1],"dataBase64":""},
        {"path":[1,1],"stackHeight":2,"programIdIndex":3,"accounts":[0,1,1],"dataBase64":STANDARD.encode(bytes)}]);
    let trace = json!({"schema":"svm-inner-instructions/v1","source":"runtime","coverage":"recorded-invocations",
        "groups":[{"index":1,"instructions":instructions}]});
    let mut archive = trace["groups"].clone();
    for entry in archive[0]["instructions"].as_array_mut().unwrap() {
        let row = entry.as_object_mut().unwrap();
        let data = STANDARD
            .decode(row.remove("dataBase64").unwrap().as_str().unwrap())
            .unwrap();
        row.remove("path");
        row.insert("data".into(), json!(bs58::encode(data).into_string()));
    }
    (trace, archive)
}
