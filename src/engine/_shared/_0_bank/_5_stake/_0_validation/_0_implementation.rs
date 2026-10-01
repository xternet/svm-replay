use super::*;

pub(in super::super) const STAKE: &str = "Stake11111111111111111111111111111111111111";

pub(in super::super) const ADJUST_RENT: &str = "BY4JhHLahVzS9ynfDz4exzGPbVXhFmJvEyMWsXbDBqME";

pub(in super::super) const ERAS: [(&str, &str, &str); 6] = [
    (
        "litesvm-v0.7.1-agave-2.3.9",
        "2.3.9",
        "47647df756f5dd0b3c739cecaa71bcf754af6be8",
    ),
    (
        "litesvm-v0.8.2-agave-3.0.10",
        "3.0.10",
        "96c3a8519a3bac8c7e7dd49b6d6aefcfeba09d90",
    ),
    (
        "litesvm-v0.12.0-agave-3.1.11",
        "3.1.11",
        "81d1f5fb0c75203329468250df3769393ab1479b",
    ),
    (
        "litesvm-v0.13.1-agave-4.0.0",
        "4.0.0",
        "2a165e7a90af75c76426d1e031ed0284211d5d1e",
    ),
    (
        "litesvm-v0.14.0-pr402-agave-4.1.2",
        "4.1.2",
        "182084b82aae88b1b0731540f63bf7416f27773a",
    ),
    (
        "litesvm-v0.16.0-agave-4.2.1",
        "4.2.1",
        "c4b48df969a9e4f121e14a389bd7bec34c752507",
    ),
];

pub(in super::super) fn receipt(source: &Value, method: &str) -> Result<Value> {
    let raw = bytes(field(source, "responseBodyBase64")?)?;
    check(
        field(source, "responseSha256")? == &json!(hash(&raw)),
        "raw response integrity",
    )?;
    let request = field(source, "request")?;
    check(
        field(source, "httpStatus")? == &json!(200)
            && field(request, "jsonrpc")? == "2.0"
            && field(request, "method")? == method
            && array(field(request, "params")?)?.len() == 2,
        "RPC request",
    )?;
    integer(field(request, "id")?)?;
    let payload = svm_replay_protocol::parse_json(&raw)?;
    check(
        field(&payload, "jsonrpc")? == "2.0"
            && field(&payload, "id")? == field(request, "id")?
            && payload.get("error").is_none(),
        "RPC response identity/error",
    )?;
    let result = field(&payload, "result")?;
    object(result)?;
    Ok(result.clone())
}

pub(in super::super) fn stake_account(source: &Value, id: &str, slot: u64) -> Result<Value> {
    let result = receipt(source, "getAccountInfo")?;
    let params = array(field(field(source, "request")?, "params")?)?;
    let options = &params[1];
    check(
        params[0] == id
            && field(options, "slot")? == &json!(slot)
            && field(options, "commitment")? == "finalized"
            && field(options, "encoding")? == "base64"
            && field(field(&result, "context")?, "slot")? == &json!(slot),
        "account boundary identity",
    )?;
    let value = field(&result, "value")?;
    check(!value.is_null(), "stake account absent")?;
    check(
        field(value, "owner")? == STAKE && field(value, "executable")? == &json!(false),
        "stake account owner/presence",
    )?;
    let tuple = array(field(value, "data")?)?;
    check(
        tuple.len() == 2 && tuple[1] == "base64",
        "stake data encoding",
    )?;
    let data = bytes(&tuple[0])?;
    check(
        data.len() == 200 && data[..4] == 2u32.to_le_bytes(),
        "StakeStateV2 layout",
    )?;
    Ok(
        json!({"pubkey":id,"sourceSlot":slot,"role":"application","presence":"present",
        "lamports":uint(field(value,"lamports")?)?.to_string(),"owner":STAKE,"executable":false,
        "rentEpoch":uint(field(value,"rentEpoch")?)?.to_string(),"dataBase64":tuple[0]}),
    )
}

pub(in super::super) fn read_u64(data: &[u8], offset: usize) -> Result<u64> {
    let data = data
        .get(offset..offset + 8)
        .ok_or_else(|| fail("truncated u64 field"))?;
    Ok(u64::from_le_bytes(
        data.try_into()
            .map_err(|e| fail(format!("u64 bytes: {e}")))?,
    ))
}

pub(in super::super) fn phase_bytes(
    account: &Value,
    id: &str,
    slot: u64,
    length: usize,
) -> Result<Vec<u8>> {
    check(
        field(account, "pubkey")? == id
            && field(account, "sourceSlot")? == &json!(slot)
            && field(account, "role")? == "sysvar"
            && field(account, "presence")? == "present"
            && field(account, "owner")? == sysvars::SYSVAR_OWNER
            && field(account, "executable")? == &json!(false),
        "exact target sysvar identity",
    )?;
    uint(field(account, "lamports")?)?;
    uint(field(account, "rentEpoch")?)?;
    let data = bytes(field(account, "dataBase64")?)?;
    check(data.len() == length, "exact target sysvar bytes")?;
    Ok(data)
}
