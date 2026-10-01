use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use svm_replay_engine::shared::bank::program_header::derive;

const LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
fn key(n: u8) -> String {
    bs58::encode([n; 32]).into_string()
}
fn account(key: &str, slot: u64, data: Vec<u8>, executable: bool) -> Value {
    json!({"pubkey":key,"sourceSlot":slot,"role":"application","presence":"present",
        "owner":LOADER,"executable":executable,"lamports":"999999999",
        "rentEpoch":u64::MAX.to_string(),"dataBase64":STANDARD.encode(data)})
}
fn fixture() -> Value {
    let mut header = 2u32.to_le_bytes().to_vec();
    header.extend([8; 32]);
    let mut code = 3u32.to_le_bytes().to_vec();
    code.extend(20u64.to_le_bytes());
    code.extend([0; 33]);
    let mut rent = 3480u64.to_le_bytes().to_vec();
    rent.extend(2f64.to_le_bytes());
    rent.push(50);
    let mut rent = account(
        "SysvarRent111111111111111111111111111111111",
        40,
        rent,
        false,
    );
    rent["owner"] = json!("Sysvar1111111111111111111111111111111111111");
    let tx = json!({"transaction":{"message":{"accountKeys":[key(9),key(7),key(8),key(10),key(11),
        "SysvarRent111111111111111111111111111111111","SysvarC1ock11111111111111111111111111111111",LOADER],
        "header":{"numRequiredSignatures":1,"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":3},
        "instructions":[{"programIdIndex":7,"accounts":[2,1,3,4,5,6,0],"data":bs58::encode(3u32.to_le_bytes()).into_string()}]}},
        "meta":{"err":null,"preBalances":[1000000,1141440,1,1,1,1,1,1],"postBalances":[995000,1141440,1,1,1,1,1,1]}});
    json!({"program":key(7),"parentSlot":40,"targetSlot":41,"anchor":account(&key(7),60,header,true),
        "programdata":account(&key(8),40,code,false),"rent":rent,"witnessSlot":20,
        "witnessBlock":{"transactions":[tx.clone()]},"targetBlock":{"parentSlot":40,"transactions":[tx],"rewards":[]},
        "registry":{"window":{"earliestSlot":10,"latestSlot":100},
        "features":[{"id":"2aQJYqER2aKyb3cZw22v4SL2xMX7vwXBRWfvS4pTrtED","activationSlot":null},
        {"id":"CJzY83ggJHqPGDq8VisV3U91jDJLuEaALZooBrXtnnLU","activationSlot":1}],
        "profiles":[{"earliestSlot":10,"latestSlot":100,"executorSourceId":"litesvm-v0.6.1-agave-2.2.20"}]}})
}

#[test]
fn reconstructs_exact_fields_without_future_balance() {
    let input = fixture();
    let result = derive(&input).unwrap();
    assert_eq!(result["account"]["lamports"], "1141440");
    assert_eq!(result["account"]["sourceSlot"], 40);
    assert_eq!(result["account"]["rentEpoch"], u64::MAX.to_string());
    assert_eq!(
        result["account"]["dataBase64"],
        input["anchor"]["dataBase64"]
    );
    assert_eq!(result["proof"]["schema"], "historical-program-header/v1");
}

#[test]
fn wrong_incomplete_and_ambiguous_evidence_rejects() {
    for (pointer, value) in [
        ("/anchor/owner", json!(key(7))),
        ("/anchor/executable", json!(false)),
        ("/anchor/sourceSlot", json!(39)),
        ("/programdata/sourceSlot", json!(41)),
        ("/programdata/pubkey", json!(key(6))),
        ("/rent/sourceSlot", json!(41)),
        ("/rent/owner", json!(LOADER)),
        ("/registry/features/0/activationSlot", json!(50)),
        ("/registry/features/1/activationSlot", json!(30)),
        ("/registry/profiles/0/executorSourceId", json!("unknown")),
        ("/witnessBlock/transactions/0/meta/err", json!("failed")),
        ("/witnessBlock/transactions/0/meta/preBalances/1", json!(1)),
        (
            "/witnessBlock/transactions/0/transaction/message/header/numReadonlyUnsignedAccounts",
            json!(7),
        ),
        ("/targetBlock/parentSlot", json!(39)),
        ("/targetBlock/transactions", json!([])),
        (
            "/targetBlock/rewards",
            json!([{"pubkey":key(7),"lamports":1}]),
        ),
        ("/witnessSlot", json!(19)),
        ("/witnessBlock/transactions", json!([])),
    ] {
        let mut input = fixture();
        *input.pointer_mut(pointer).unwrap() = value;
        assert!(derive(&input).is_err(), "accepted {pointer}");
    }
    let mut input = fixture();
    let duplicate = input["witnessBlock"]["transactions"][0].clone();
    input["witnessBlock"]["transactions"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    assert!(derive(&input).is_err());
}

#[test]
fn bank_managed_programs_require_separate_proof() {
    let mut input = fixture();
    let program = "Stake11111111111111111111111111111111111111";
    input["program"] = json!(program);
    input["anchor"]["pubkey"] = json!(program);
    for block in ["targetBlock", "witnessBlock"] {
        input[block]["transactions"][0]["transaction"]["message"]["accountKeys"][1] =
            json!(program);
    }
    assert!(derive(&input).is_err());
}

#[test]
fn earlier_witness_requires_identical_recorded_features() {
    let mut input = fixture();
    input["registry"]["window"]["earliestSlot"] = json!(30);
    input["registry"]["profiles"][0]["earliestSlot"] = json!(30);
    assert!(derive(&input).is_ok());
    input["registry"]["features"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":key(17),"activationSlot":25}));
    assert!(derive(&input).is_err());
}
