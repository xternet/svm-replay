use super::*;

pub(super) const LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

pub(super) const RENT: &str = "SysvarRent111111111111111111111111111111111";

pub(super) const ROWS: [(&str, &str, &str, &str, &str); 4] = [
    (
        "ptokFjwyJtrwCa9Kgo9xoDS59V4QccBGEaRFnRPnSdP",
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "3gvYRKWyXRR9xKWe1ZjPhLY5ZJRN7KDB4rFZFGoJfFk2",
        "ptok6rngomXrDbWf5v5Mkmu5CEbB51hzSCPDoj9DrvF",
        "litesvm-v0.12.0-agave-3.1.11",
    ),
    (
        "Gx4XFcrVMt4HUvPzTpTSVkdDVgcDSjKhDN1RqRS6KDuZ",
        "Stake11111111111111111111111111111111111111",
        "6WU8Nxarf9fudRK5atWwjLY4vFaw5UrrWhL88qz7iCMJ",
        "BM11F4hqrpinQs28sEZfzQ2fYddivYs4NEAHF6QMjkJF",
        "litesvm-v0.12.0-agave-3.1.11",
    ),
    (
        "STk5Xj8hdAx3sTzmtJ3QysKkq6X2A3yj73JtxttiRyk",
        "Stake11111111111111111111111111111111111111",
        "6WU8Nxarf9fudRK5atWwjLY4vFaw5UrrWhL88qz7iCMJ",
        "4EBQBjw1kqF1dqUBb6fc5Ji4tCEQgNf9ESGGX3smwXwh",
        "litesvm-v0.13.1-agave-4.0.0",
    ),
    (
        "s51VGwCAgebo2745DSUris72RavoLkXGUmVJosESCXr",
        "Stake11111111111111111111111111111111111111",
        "6WU8Nxarf9fudRK5atWwjLY4vFaw5UrrWhL88qz7iCMJ",
        "p51x11QCYMHwuVS1MBcLHKb3MezWyqGS5BEB41CA1dk",
        "litesvm-v0.16.0-agave-4.2.1",
    ),
];

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

pub(super) fn body(receipt: &Value) -> Value {
    svm_replay_protocol::parse_json(
        &STANDARD
            .decode(receipt["responseBodyBase64"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap()
}

pub(super) fn change(receipt: &mut Value, edit: impl FnOnce(&mut Value)) {
    let mut value = body(receipt);
    edit(&mut value);
    let raw = serde_json::to_vec(&value).unwrap();
    receipt["responseBodyBase64"] = json!(STANDARD.encode(&raw));
    receipt["responseSha256"] = json!(hash(raw));
}

pub(super) fn change_bytes(receipt: &mut Value, edit: impl FnOnce(&mut Vec<u8>)) {
    change(receipt, |value| {
        let mut bytes = STANDARD
            .decode(value["result"]["value"]["data"][0].as_str().unwrap())
            .unwrap();
        edit(&mut bytes);
        value["result"]["value"]["data"][0] = json!(STANDARD.encode(bytes));
    });
}

pub(super) fn image(owner: &str, executable: bool, lamports: u64, data: &[u8]) -> Value {
    json!({"owner":owner,"executable":executable,"lamports":lamports,"rentEpoch":u64::MAX,"data":[STANDARD.encode(data),"base64"]})
}

pub(super) fn account(id: &str, slot: u64, value: Value) -> Value {
    rpc(
        "getAccountInfo",
        json!([id,{"slot":slot,"encoding":"base64","commitment":"finalized"}]),
        json!({"context":{"slot":slot},"value":value}),
    )
}

pub(super) fn input(index: usize, authority: bool) -> Value {
    let (feature, program, pd, buffer, executor) = ROWS[index];
    let mut program_bytes = 2u32.to_le_bytes().to_vec();
    program_bytes.extend(bs58::decode(pd).into_vec().unwrap());
    let mut old_pd = vec![0u8; 48];
    old_pd[..4].copy_from_slice(&3u32.to_le_bytes());
    old_pd[4..12].copy_from_slice(&18u64.to_le_bytes());
    let mut source = vec![0u8; 40];
    source[..4].copy_from_slice(&1u32.to_le_bytes());
    source[37..].copy_from_slice(&[7, 8, 9]);
    if authority {
        old_pd[12] = 1;
        old_pd[13..45].fill(6);
        source[4] = 1;
        source[5..37].fill(6);
    }
    let mut new_pd = old_pd.clone();
    new_pd[4..12].copy_from_slice(&20u64.to_le_bytes());
    new_pd[45..].copy_from_slice(&source[37..]);
    if index == 0 {
        new_pd[12..45].fill(0);
    }
    let mut rent = [0u8; 17];
    rent[..8].copy_from_slice(&2u64.to_le_bytes());
    rent[8..16].copy_from_slice(&2f64.to_le_bytes());
    let entries=[(program,if index==0{image("BPFLoader2111111111111111111111111111111111",true,1000,&[1,2,3])}else{image(LOADER,true,656,&program_bytes)},image(LOADER,true,656,&program_bytes)),
        (pd,if index==0{Value::Null}else{image(LOADER,false,704,&old_pd)},image(LOADER,false,704,&new_pd)),
        (buffer,image(LOADER,false,1000,&source),Value::Null)].into_iter().map(|(id,parent,target)|json!({"pubkey":id,"parentAccount":account(id,19,parent),"targetAccount":account(id,20,target)})).collect::<Vec<_>>();
    let block = rpc(
        "getBlock",
        json!([20,{"commitment":"finalized","encoding":"json","transactionDetails":"full","rewards":true,"maxSupportedTransactionVersion":0}]),
        json!({"parentSlot":19,"transactions":[],"rewards":[]}),
    );
    json!({"slot":20,"parentSlot":19,"blockSourceHash":block["responseSha256"],"envelope":body(&block),
        "runtime":{"executor":{"id":executor},"features":[{"id":feature,"activationSlot":20}]},"requestedPubkeys":[program],
        "evidence":{"evidenceKind":"controlled-bank","block":block,"accounts":entries,"rent":account(RENT,20,image("Sysvar1111111111111111111111111111111111111",false,1,&rent))}})
}

pub(super) fn strict_rent(input: &Value) -> Value {
    let receipt = &input["evidence"]["rent"];
    let value = body(receipt)["result"]["value"].clone();
    json!({"pubkey":RENT,"sourceSlot":input["slot"],"role":"sysvar","presence":"present","owner":value["owner"],
        "executable":value["executable"],"lamports":value["lamports"].as_u64().unwrap().to_string(),"rentEpoch":value["rentEpoch"].as_u64().unwrap().to_string(),"dataBase64":value["data"][0]})
}

pub(super) fn fixture(input: &Value, context: &Value) -> Value {
    let mut accounts = context["initialized"].as_array().unwrap().clone();
    accounts.push(strict_rent(input));
    json!({"target":{"targetSlot":input["slot"],"parentSlot":input["parentSlot"]},"runtime":{"binding":input["runtime"],
        "bankContext":{"blockSourceHash":input["blockSourceHash"]},"programMigrationProofs":context["proofs"]},"accounts":accounts})
}
