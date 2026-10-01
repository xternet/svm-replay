use super::*;

pub(super) const LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

pub(super) const V4: &str = "LoaderV411111111111111111111111111111111111";

pub(super) fn key(n: u8) -> String {
    bs58::encode([n; 32]).into_string()
}

pub(super) fn account(n: u8, tag: u32) -> Value {
    let mut data = vec![0; if tag == 2 { 36 } else { 46 }];
    data[..4].copy_from_slice(&tag.to_le_bytes());
    if tag == 2 {
        data[4..36].copy_from_slice(&[2; 32]);
    }
    if tag == 3 {
        data[4..12].copy_from_slice(&99u64.to_le_bytes());
        data[12] = 1;
        data[13..45].copy_from_slice(&[4; 32]);
    }
    if tag == 1 {
        data[4] = 1;
        data[5..37].copy_from_slice(&[4; 32]);
    }
    json!({"pubkey":key(n),"sourceSlot":100,"role":if tag==2{"program"}else{"application"},"presence":"present","owner":LOADER,"executable":tag==2,"lamports":"9007199254740993","rentEpoch":u64::MAX.to_string(),"dataBase64":STANDARD.encode(data)})
}

pub(super) fn absent(n: u8) -> Value {
    json!({"pubkey":key(n),"sourceSlot":100,"role":"application","presence":"absent"})
}

pub(super) fn context() -> HistoricalLoaderContext {
    HistoricalLoaderContext {
        parent_slot: 100,
        accounts: vec![account(1, 2), account(2, 3), account(3, 1)],
        included_transactions: vec![],
    }
}

pub(super) fn ix(tag: u32) -> Value {
    let mut data = vec![
        0;
        match tag {
            1 => 19,
            2 => 12,
            6 => 8,
            _ => 4,
        }
    ];
    data[..4].copy_from_slice(&tag.to_le_bytes());
    if tag == 1 {
        data[8..16].copy_from_slice(&3u64.to_le_bytes());
        data[16..].copy_from_slice(&[7, 8, 9]);
    }
    let accounts = match tag {
        0 | 1 | 4 | 7 => vec![2, 3, 4],
        2 => vec![3, 1, 0, 2, 5, 6, 7, 3],
        3 => vec![1, 0, 2, 4, 5, 6, 3],
        5 => vec![1, 4, 3, 0],
        6 => vec![1, 0],
        _ => vec![2, 3],
    };
    json!({"programIdIndex":8,"accounts":accounts,"data":bs58::encode(data).into_string()})
}

pub(super) fn raw(tags: &[u32], inner: bool) -> Value {
    let instructions: Vec<_> = tags.iter().map(|t| ix(*t)).collect();
    json!({"transaction":{"signatures":["controlled"],"message":{"header":{"numRequiredSignatures":1,"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":1},"accountKeys":[key(1),key(2),key(3),key(4),key(5),"SysvarRent111111111111111111111111111111111","SysvarC1ock11111111111111111111111111111111",key(6),LOADER],"instructions":if inner{vec![json!({"programIdIndex":7,"accounts":[0,1,2,3,4,5,6,8],"data":""})]}else{instructions.clone()}}},"meta":{"err":null,"loadedAddresses":{"writable":[],"readonly":[]},"innerInstructions":if inner{vec![json!({"index":0,"instructions":instructions})]}else{vec![]}}})
}

pub(super) fn inspect(
    raw: &Value,
    c: &HistoricalLoaderContext,
) -> Result<(), svm_replay_protocol::Error> {
    assert_inspected_loader_lifecycle(raw, 1, c)
}
