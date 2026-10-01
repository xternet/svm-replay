use super::*;

pub(super) fn hash(v: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(v.as_ref()))
}

pub(super) fn key(n: u8) -> String {
    bs58::encode([n; 32]).into_string()
}

pub(super) fn rpc(method: &str, params: Value, result: Value) -> Value {
    let raw = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"result":result})).unwrap();
    json!({"request":{"jsonrpc":"2.0","id":1,"method":method,"params":params},"httpStatus":200,
        "responseBodyBase64":STANDARD.encode(&raw),"responseSha256":hash(raw)})
}

pub(super) fn payload(receipt: &Value) -> Value {
    svm_replay_protocol::parse_json(
        &STANDARD
            .decode(receipt["responseBodyBase64"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap()
}

pub(super) fn change(receipt: &mut Value, edit: impl FnOnce(&mut Value)) {
    let mut value = payload(receipt);
    edit(&mut value);
    let raw = serde_json::to_vec(&value).unwrap();
    receipt["responseBodyBase64"] = json!(STANDARD.encode(&raw));
    receipt["responseSha256"] = json!(hash(raw));
}

pub(super) fn bytes(receipt: &Value) -> Vec<u8> {
    STANDARD
        .decode(
            payload(receipt)["result"]["value"]["data"][0]
                .as_str()
                .unwrap(),
        )
        .unwrap()
}

pub(super) fn change_bytes(receipt: &mut Value, edit: impl FnOnce(&mut Vec<u8>)) {
    let mut data = bytes(receipt);
    edit(&mut data);
    change(receipt, |v| {
        v["result"]["value"]["data"][0] = json!(STANDARD.encode(data))
    });
}

pub(super) fn instruction(kind: &str) -> Value {
    let loaded = kind == "loaded-readonly" || kind == "loaded-writer";
    let keys = if kind == "payer" {
        json!([key(1), key(2), "11111111111111111111111111111111"])
    } else if loaded {
        json!([key(2), "11111111111111111111111111111111"])
    } else if kind == "untouched" {
        json!([key(2), key(7), "11111111111111111111111111111111"])
    } else {
        json!([key(2), key(1), "11111111111111111111111111111111"])
    };
    let mut value = json!({"version":if loaded{json!(0)}else{json!("legacy")},"transaction":{"signatures":[bs58::encode([8;64]).into_string()],
        "message":{"accountKeys":keys,"header":{"numRequiredSignatures":1,"numReadonlySignedAccounts":0,
            "numReadonlyUnsignedAccounts":if kind=="readonly"{2}else{1}},
            "instructions":[{"programIdIndex":if loaded{1}else{2},"accounts":if kind=="nonce"{json!([1,0])}else if loaded{json!([2])}else{json!([1])},
                "data":if kind=="nonce"{bs58::encode([4,0,0,0]).into_string()}else{String::new()}}]}},
        "meta":{"err":if ["failed-writer","payer","nonce"].contains(&kind){json!({"InstructionError":[0,"InvalidArgument"]})}else{Value::Null},
            "innerInstructions":[],"loadedAddresses":{"writable":[],"readonly":if loaded{json!([key(1)])}else{json!([])}}}});
    if loaded {
        value["transaction"]["message"]["addressTableLookups"] =
            json!([{"accountKey":key(9),"writableIndexes":[],"readonlyIndexes":[0]}]);
    }
    if kind == "loaded-writer" {
        value["transaction"]["message"]["addressTableLookups"][0]["writableIndexes"] = json!([0]);
        value["transaction"]["message"]["addressTableLookups"][0]["readonlyIndexes"] = json!([]);
        value["meta"]["loadedAddresses"] = json!({"writable":[key(1)],"readonly":[]});
    }
    value
}

pub(super) fn input(zero: bool) -> Value {
    let reward = if zero { 0 } else { 7 };
    let mut old = vec![0u8; 200];
    old[..4].copy_from_slice(&2u32.to_le_bytes());
    old[156..164].copy_from_slice(&5000u64.to_le_bytes());
    old[172..180].copy_from_slice(&u64::MAX.to_le_bytes());
    old[188..196].copy_from_slice(&1u64.to_le_bytes());
    let mut new = old.clone();
    new[156..164].copy_from_slice(&(5000u64 + reward).to_le_bytes());
    new[188..196].copy_from_slice(&2u64.to_le_bytes());
    let snapshot = |slot, data: Vec<u8>, lamports| {
        rpc(
            "getAccountInfo",
            json!([key(1),{"slot":slot,"commitment":"finalized","encoding":"base64"}]),
            json!({"context":{"slot":slot},"value":{"lamports":lamports,"owner":"Stake11111111111111111111111111111111111111",
            "executable":false,"rentEpoch":u64::MAX,"data":[STANDARD.encode(data),"base64"]}}),
        )
    };
    json!({"pubkey":key(1),"boundary":{"slot":20,"parentSlot":19,"blockhash":key(3),"blockHeight":10,"transactionCount":1},
        "runtime":{"executorSourceId":"litesvm-v0.13.1-agave-4.0.0","activeExecutionFeatureIds":[]},
        "block":rpc("getBlock",json!([20,{"commitment":"finalized","encoding":"json","transactionDetails":"full","rewards":true,"maxSupportedTransactionVersion":0}]),
            json!({"parentSlot":19,"blockhash":key(3),"previousBlockhash":key(4),"blockHeight":10,"transactions":[instruction("readonly")],
                "rewards":if zero{json!([])}else{json!([{"pubkey":key(1),"rewardType":"Staking","lamports":reward,"postBalance":10000+reward}])}})),
        "parentBlock":rpc("getBlock",json!([19,{"commitment":"finalized"}]),json!({"blockhash":key(4),"blockHeight":9})),
        "parentAccount":snapshot(19,old,10000),"targetAccount":snapshot(20,new,10000+reward)})
}

pub(super) fn strict(receipt: &Value, slot: u64) -> Value {
    let value = &payload(receipt)["result"]["value"];
    json!({"pubkey":key(1),"sourceSlot":slot,"role":"application","presence":"present","owner":value["owner"],
        "executable":value["executable"],"lamports":value["lamports"].as_u64().unwrap().to_string(),"rentEpoch":value["rentEpoch"].as_u64().unwrap().to_string(),"dataBase64":value["data"][0]})
}

pub(super) fn prepared(source: &Value) -> Value {
    json!({"accounts":[strict(&source["parentAccount"],19)],"slot":20,"parentSlot":19,
    "envelope":payload(&source["block"]),"blockSourceHash":source["block"]["responseSha256"],"runtime":source["runtime"],
    "evidence":{"block":source["block"],"parentBlock":source["parentBlock"],"accounts":[{"pubkey":source["pubkey"],"parentAccount":source["parentAccount"],"targetAccount":source["targetAccount"]}]}})
}

pub(super) fn adjusted(zero: bool, rate: u64, threshold: f64) -> Value {
    let mut source = input(zero);
    source["runtime"]["executorSourceId"] = json!("litesvm-v0.16.0-agave-4.2.1");
    source["runtime"]["activeExecutionFeatureIds"] =
        json!(["BY4JhHLahVzS9ynfDz4exzGPbVXhFmJvEyMWsXbDBqME"]);
    let mut clock = [0u8; 40];
    clock[..8].copy_from_slice(&20u64.to_le_bytes());
    clock[16..24].copy_from_slice(&4u64.to_le_bytes());
    let mut rent = [0u8; 17];
    rent[..8].copy_from_slice(&rate.to_le_bytes());
    rent[8..16].copy_from_slice(&threshold.to_le_bytes());
    let phase = |id: &str, data: &[u8]| {
        json!({"pubkey":id,"sourceSlot":20,"role":"sysvar","presence":"present",
        "owner":"Sysvar1111111111111111111111111111111111111","executable":false,"lamports":"1",
        "rentEpoch":u64::MAX.to_string(),"dataBase64":STANDARD.encode(data)})
    };
    source["phaseContext"] = json!({"clock":phase("SysvarC1ock11111111111111111111111111111111",&clock),
        "rent":phase("SysvarRent111111111111111111111111111111111",&rent)});
    source
}
