use super::*;

pub const LAST_RESTART_SLOT: &str = "SysvarLastRestartS1ot1111111111111111111111";

pub const LAST_RESTART_SOURCE: &str = "exact-historical-get-account-info-v1";

pub fn last_restart_account_hash(account: &Value) -> Result<String> {
    check(
        field(account, "presence")? == "present",
        "LastRestartSlot account absent",
    )?;
    let fields = [
        string(field(account, "pubkey")?)?.to_owned(),
        integer(field(account, "sourceSlot")?)?.to_string(),
        "present".into(),
        uint(field(account, "lamports")?)?.to_string(),
        string(field(account, "owner")?)?.into(),
        field(account, "executable")?
            .as_bool()
            .ok_or_else(|| fail("executable malformed"))?
            .to_string(),
        uint(field(account, "rentEpoch")?)?.to_string(),
        string(field(account, "dataBase64")?)?.into(),
    ];
    Ok(hash(format!(
        "svm-call-historical-account-v1\n{}\n",
        fields.join("\n")
    )))
}

pub fn last_restart_binding_hash(binding: &Value) -> Result<String> {
    let mut lines = Vec::new();
    for key in [
        "pubkey",
        "sourceSlot",
        "contextSlot",
        "owner",
        "dataBase64",
        "dataSha256",
        "embeddedSlot",
        "accountHash",
        "rawResponseSha256",
        "cacheKeyHash",
        "sourceKind",
    ] {
        let value = field(binding, key)?;
        lines.push(if key == "sourceSlot" || key == "contextSlot" {
            integer(value)?.to_string()
        } else {
            string(value)?.into()
        });
    }
    Ok(hash(format!(
        "svm-call-exact-last-restart-slot-v1\n{}\n",
        lines.join("\n")
    )))
}

pub fn assert_last_restart_slot_binding(
    account: &Value,
    binding: &Value,
    parent_slot: u64,
) -> Result<()> {
    check(
        field(account, "pubkey")? == LAST_RESTART_SLOT
            && field(account, "sourceSlot")? == &json!(parent_slot)
            && field(account, "role")? == "sysvar"
            && field(account, "presence")? == "present"
            && field(account, "owner")? == SYSVAR_OWNER
            && field(account, "executable")? == &json!(false)
            && field(binding, "pubkey")? == LAST_RESTART_SLOT
            && field(binding, "sourceSlot")? == &json!(parent_slot)
            && field(binding, "contextSlot")? == &json!(parent_slot)
            && field(binding, "owner")? == SYSVAR_OWNER
            && field(binding, "sourceKind")? == LAST_RESTART_SOURCE,
        "LastRestartSlot boundary mismatch",
    )?;
    let data = bytes(field(account, "dataBase64")?)?;
    check(
        data.len() == 8,
        "LastRestartSlot must contain exactly 8 bytes",
    )?;
    let embedded = u64::from_le_bytes(
        data.as_slice()
            .try_into()
            .map_err(|e| fail(format!("restart bytes: {e}")))?,
    )
    .to_string();
    check(
        field(binding, "dataBase64")? == field(account, "dataBase64")?
            && field(binding, "dataSha256")? == &json!(hash(data))
            && field(binding, "embeddedSlot")? == &json!(embedded)
            && field(binding, "accountHash")? == &json!(last_restart_account_hash(account)?)
            && field(binding, "cacheKeyHash")?
                == &json!(hash(format!(
                    "solana-mainnet\n{parent_slot}\n{LAST_RESTART_SLOT}\n"
                )))
            && field(binding, "bindingHash")? == &json!(last_restart_binding_hash(binding)?),
        "LastRestartSlot binding hash mismatch",
    )?;
    for key in [
        "dataSha256",
        "accountHash",
        "rawResponseSha256",
        "cacheKeyHash",
        "bindingHash",
    ] {
        digest(string(field(binding, key)?)?)?;
    }
    Ok(())
}

/// Build from a supplied, raw-byte-backed recorded singular response; no network or invented transcript.
pub fn prepare_last_restart_slot(transcript: &Value, parent_slot: u64) -> Result<Value> {
    check(
        field(transcript, "schema")? == "svm-call-historical-account-transcript-v1",
        "historical account transcript schema mismatch",
    )?;
    let request = field(transcript, "request")?;
    check(
        field(request, "method")? == "getAccountInfo" && field(request, "id")? == &json!(0),
        "LastRestartSlot singular request malformed",
    )?;
    let params = array(field(request, "params")?)?;
    check(
        params.len() == 2
            && params[0] == LAST_RESTART_SLOT
            && params[1]
                == json!({"commitment":"finalized","encoding":"base64","slot":parent_slot}),
        "LastRestartSlot historical request boundary mismatch",
    )?;
    let raw = bytes(field(transcript, "responseBodyBase64")?)?;
    check(
        integer(field(transcript, "responseBytes")?)? == raw.len() as u64
            && field(transcript, "responseSha256")? == &json!(hash(&raw)),
        "LastRestartSlot raw response identity mismatch",
    )?;
    let parsed = svm_replay_protocol::parse_json(&raw)?;
    let responses = array(&parsed)?;
    check(
        responses.len() == 1,
        "LastRestartSlot response is not singular",
    )?;
    let response = &responses[0];
    check(
        field(response, "jsonrpc")? == "2.0" && field(response, "id")? == &json!(0),
        "LastRestartSlot response version/id mismatch",
    )?;
    if let Some(error) = response.get("error") {
        return Err(fail(format!("LastRestartSlot RPC error: {error}")));
    }
    let result = field(response, "result")?;
    check(
        field(field(result, "context")?, "slot")? == &json!(parent_slot),
        "LastRestartSlot historical context drift",
    )?;
    let value = field(result, "value")?;
    check(
        !value.is_null(),
        "LastRestartSlot absent at historical parent",
    )?;
    let tuple = array(field(value, "data")?)?;
    check(
        tuple.len() == 2 && tuple[1] == "base64",
        "LastRestartSlot historical data encoding malformed",
    )?;
    let data = bytes(&tuple[0])?;
    check(
        data.len() == 8
            && field(value, "owner")? == SYSVAR_OWNER
            && field(value, "executable")? == &json!(false),
        "LastRestartSlot canonical account constraints failed",
    )?;
    let account = json!({"pubkey":LAST_RESTART_SLOT,"sourceSlot":parent_slot,"role":"sysvar","presence":"present",
        "lamports":uint(field(value,"lamports")?)?.to_string(),"owner":SYSVAR_OWNER,"executable":false,
        "rentEpoch":uint(field(value,"rentEpoch")?)?.to_string(),"dataBase64":tuple[0]});
    let embedded = u64::from_le_bytes(
        data.as_slice()
            .try_into()
            .map_err(|e| fail(format!("restart bytes: {e}")))?,
    )
    .to_string();
    let cache_key = hash(format!(
        "solana-mainnet\n{parent_slot}\n{LAST_RESTART_SLOT}\n"
    ));
    let mut binding = json!({"pubkey":LAST_RESTART_SLOT,"sourceSlot":parent_slot,"contextSlot":parent_slot,"owner":SYSVAR_OWNER,
        "dataBase64":tuple[0],"dataSha256":hash(data),"embeddedSlot":embedded,"accountHash":last_restart_account_hash(&account)?,
        "rawResponseSha256":field(transcript,"responseSha256")?,"cacheKeyHash":cache_key,"sourceKind":LAST_RESTART_SOURCE});
    binding["bindingHash"] = json!(last_restart_binding_hash(&binding)?);
    assert_last_restart_slot_binding(&account, &binding, parent_slot)?;
    Ok(
        json!({"account":account,"binding":binding,"transcript":transcript,
        "cache":{"status":"RECORDED_TRANSCRIPT","keyHash":cache_key,"valueHash":binding["accountHash"]}}),
    )
}
