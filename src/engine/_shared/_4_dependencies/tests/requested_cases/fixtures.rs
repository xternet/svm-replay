use super::*;

pub(super) const ALT: &str = "AddressLookupTab1e1111111111111111111111111";

pub(super) const LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

pub(super) fn key(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

pub(super) fn wire(with_table: bool, index: u8) -> String {
    let mut bytes = vec![1];
    bytes.extend([7; 64]);
    bytes.extend([128, 1, 0, 1, 2]);
    bytes.extend([1; 32]);
    bytes.extend([0; 32]);
    bytes.extend([8; 32]);
    bytes.extend(if with_table {
        vec![1, 1, 2, 0, 2, 0]
    } else {
        vec![1, 1, 1, 0, 0]
    });
    if with_table {
        bytes.push(1);
        bytes.extend([3; 32]);
        bytes.extend([1, index, 0]);
    } else {
        bytes.push(0);
    }
    STANDARD.encode(bytes)
}

pub(super) fn original(with_table: bool) -> Value {
    let mut value =
        svm_replay_protocol::transaction::decode(&wire(with_table, 0)).expect("real wire");
    value["meta"] = json!({"err":null,"innerInstructions":[],"loadedAddresses":{"writable":if with_table{vec![key(2)]}else{vec![]},"readonly":[]}});
    value
}

pub(super) fn table(address: u8, last: u64, start: u8) -> String {
    let mut bytes = vec![0_u8; 88];
    bytes[..4].copy_from_slice(&1u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&u64::MAX.to_le_bytes());
    bytes[12..20].copy_from_slice(&last.to_le_bytes());
    bytes[20] = start;
    bytes[56..88].copy_from_slice(&[address; 32]);
    STANDARD.encode(bytes)
}

pub(super) fn account(owner: &str, data: &str) -> Value {
    json!({"pubkey":key(3),"sourceSlot":99,"role":"application","presence":"present","owner":owner,"executable":false,"lamports":"2000000","rentEpoch":"0","dataBase64":data})
}

pub(super) fn absent() -> Value {
    json!({"pubkey":key(3),"sourceSlot":99,"role":"application","presence":"absent"})
}

pub(super) fn edit(data: &str) -> Vec<RequestedAccountOverride> {
    validate_requested_overrides(&json!([{"pubkey":key(3),"dataBase64":data}]))
        .expect("valid override")
}

pub(super) fn boundary(raws: Vec<Value>) -> RequestedBoundaryEvidence {
    RequestedBoundaryEvidence {
        target_slot: 100,
        raw_transactions: raws,
    }
}

pub(super) fn mutation(tag: u32, failed: bool, inner: bool) -> Value {
    let mut data = vec![
        0_u8;
        if tag == 0 {
            13
        } else if tag == 2 {
            44
        } else {
            4
        }
    ];
    data[..4].copy_from_slice(&tag.to_le_bytes());
    if tag == 0 {
        data[4..12].copy_from_slice(&99u64.to_le_bytes());
    }
    if tag == 2 {
        data[4..12].copy_from_slice(&1u64.to_le_bytes());
        data[12..].copy_from_slice(&[5; 32]);
    }
    let instruction = json!({"programIdIndex":3,"accounts":if tag==0 {vec![1,0,0,2]}else{vec![1,0,0]},"data":bs58::encode(data).into_string()});
    let mut keys = vec![key(1), key(3), key(0), ALT.into()];
    if inner {
        keys.push(key(8));
    }
    json!({"transaction":{"signatures":["controlled-writer"],"message":{"header":{"numRequiredSignatures":1,"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":if inner{3}else{2}},"accountKeys":keys,"instructions":if inner{json!([{"programIdIndex":4,"accounts":[0,1,2,3],"data":""}])}else{json!([instruction])}}},"meta":{"err":if failed{json!({"InstructionError":[0,"InvalidArgument"]})}else{Value::Null},"loadedAddresses":{"writable":[],"readonly":[]},"innerInstructions":if inner{json!([{"index":0,"instructions":[instruction]}])}else{json!([])}}})
}

pub(super) fn summaries(raws: &[Value]) -> Vec<SemanticTransaction> {
    raws.iter()
        .enumerate()
        .map(|(i, r)| summarize_semantic_transaction(r, i as u64).expect("summary"))
        .collect()
}

pub(super) fn resolve(
    original: &Value,
    raws: &[Value],
    parent: Value,
    edits: &[RequestedAccountOverride],
    index: u8,
) -> Result<
    svm_replay_engine::shared::dependencies::requested::RequestedResolution,
    svm_replay_protocol::Error,
> {
    resolve_requested_dependencies_with_accounts(
        &wire(true, index),
        original,
        &summaries(raws),
        raws.len() as u64,
        99,
        |keys, slot| {
            assert_eq!(keys, &[key(3)]);
            assert_eq!(slot, 99);
            Ok(vec![parent.clone()])
        },
        Some(&boundary(raws.to_vec())),
        edits,
    )
}

pub(super) fn programs() -> Vec<Value> {
    let mut metadata = vec![0_u8; 45];
    metadata[..4].copy_from_slice(&3u32.to_le_bytes());
    metadata[4..12].copy_from_slice(&99u64.to_le_bytes());
    metadata.extend(b"\x7fELFcontrol");
    let mut program = account(
        LOADER,
        &STANDARD.encode([vec![2, 0, 0, 0], vec![82; 32]].concat()),
    );
    program["pubkey"] = json!(key(81));
    program["role"] = json!("program");
    program["executable"] = json!(true);
    let mut backing = account(LOADER, &STANDARD.encode(metadata));
    backing["pubkey"] = json!(key(82));
    backing["role"] = json!("programdata");
    vec![program, backing]
}

pub(super) fn roles() -> Roles {
    let mut roles = Roles::default();
    roles.programs.extend([ALT.into(), key(0)]);
    roles.address_tables.insert(key(3));
    roles
}
