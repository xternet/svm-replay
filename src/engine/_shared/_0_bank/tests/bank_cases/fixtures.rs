use super::*;

pub(super) fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

pub(super) fn key(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

pub(super) fn account(id: &str, slot: u64, data: &[u8]) -> Value {
    json!({"pubkey":id,"sourceSlot":slot,"role":"sysvar","presence":"present","lamports":"1",
        "owner":"Sysvar1111111111111111111111111111111111111","executable":false,
        "rentEpoch":u64::MAX.to_string(),"dataBase64":STANDARD.encode(data)})
}

pub(super) fn clock(slot: u64) -> Value {
    let mut data = [0u8; 40];
    data[..8].copy_from_slice(&slot.to_le_bytes());
    account(CLOCK_SYSVAR, slot, &data)
}

pub(super) fn transaction(program: &str, fee: u64, signatures: u64) -> Value {
    json!({"transaction":{"signatures":[key(7)],"message":{"header":{"numRequiredSignatures":signatures},
        "accountKeys":[key(2),program],"instructions":[{"programIdIndex":1,"accounts":[0],"data":""}]}},
        "meta":{"err":null,"fee":fee,"loadedAddresses":{"writable":[],"readonly":[]},"innerInstructions":[]}})
}

pub(super) fn block_context() -> (Value, Value, String) {
    let block = json!({"result":{"parentSlot":19,"blockhash":key(3),"previousBlockhash":key(4),"transactions":[
        transaction("Vote111111111111111111111111111111111111111",10,2),
        transaction("Vote111111111111111111111111111111111111111",5,1),
        transaction("11111111111111111111111111111111",5,1)]}});
    let selector =
        json!({"slot":20,"parentSlot":19,"blockhash":key(3),"index":2,"signature":key(7)});
    let source = hash(serde_json::to_vec(&block).unwrap());
    (block, selector, source)
}

pub(super) fn transcript() -> Value {
    let bytes = STANDARD.encode(17u64.to_le_bytes());
    let raw = format!(
        r#"[{{"jsonrpc":"2.0","id":0,"result":{{"context":{{"slot":19}},"value":{{"lamports":1,"owner":"Sysvar1111111111111111111111111111111111111","executable":false,"rentEpoch":18446744073709551615,"data":["{bytes}","base64"]}}}}}}]"#
    );
    json!({"schema":"svm-call-historical-account-transcript-v1","request":{"jsonrpc":"2.0","id":0,"method":"getAccountInfo",
        "params":[LAST_RESTART_SLOT,{"commitment":"finalized","encoding":"base64","slot":19}]},
        "responseBodyBase64":STANDARD.encode(raw.as_bytes()),"responseBytes":raw.len(),"responseSha256":hash(raw)})
}
