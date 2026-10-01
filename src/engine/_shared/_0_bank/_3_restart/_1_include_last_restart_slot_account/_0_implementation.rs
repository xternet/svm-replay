use super::*;

pub fn include_last_restart_slot_account(
    accounts: &[Value],
    account: &Value,
    binding: &Value,
    parent_slot: u64,
) -> Result<Vec<Value>> {
    check(
        !accounts
            .iter()
            .any(|value| value.get("pubkey") == Some(&json!(LAST_RESTART_SLOT))),
        "duplicate LastRestartSlot fixture account",
    )?;
    assert_last_restart_slot_binding(account, binding, parent_slot)?;
    let mut values = accounts.to_vec();
    values.push(account.clone());
    Ok(values)
}

/// A new, explicitly labeled local compatibility projection, never an original provider receipt.
pub fn prepare_last_restart_from_observation(
    account: &Value,
    evidence_hashes: &[String],
) -> Result<Value> {
    let expected = [
        "pubkey",
        "sourceSlot",
        "role",
        "presence",
        "lamports",
        "owner",
        "executable",
        "rentEpoch",
        "dataBase64",
    ];
    let fields = object(account)?;
    check(
        fields.len() == expected.len() && expected.iter().all(|key| fields.contains_key(*key)),
        "strict source account fields differ",
    )?;
    check(
        field(account, "pubkey")? == LAST_RESTART_SLOT,
        "source account is not LastRestartSlot",
    )?;
    let slot = integer(field(account, "sourceSlot")?)?;
    super::super::super::sysvars::bind_exact_generic_sysvar(account, slot)?;
    check(
        bytes(field(account, "dataBase64")?)?.len() == 8,
        "LastRestartSlot must contain exactly 8 bytes",
    )?;
    for key in ["lamports", "rentEpoch"] {
        string(field(account, key)?)?;
        uint(field(account, key)?)?;
    }
    check(
        !evidence_hashes.is_empty(),
        "retained source evidence hashes required",
    )?;
    for evidence in evidence_hashes {
        digest(evidence)?;
    }
    let response = json!([{"jsonrpc":"2.0","id":0,"result":{"context":{"slot":slot},"value":{
        "lamports":uint(field(account,"lamports")?)?,"owner":SYSVAR_OWNER,"executable":false,
        "rentEpoch":uint(field(account,"rentEpoch")?)?,"data":[field(account,"dataBase64")?,"base64"]}}}]);
    let raw = serde_json::to_vec(&response).map_err(|e| fail(format!("source projection: {e}")))?;
    let transcript = json!({"schema":"svm-call-historical-account-transcript-v1","request":{"jsonrpc":"2.0","id":0,
        "method":"getAccountInfo","params":[LAST_RESTART_SLOT,{"commitment":"finalized","encoding":"base64","slot":slot}]},
        "responseBodyBase64":STANDARD.encode(&raw),"responseBytes":raw.len(),"responseSha256":hash(&raw)});
    let mut result = prepare_last_restart_slot(&transcript, slot)?;
    check(
        field(&result, "account")? == account,
        "projection changed retained source account",
    )?;
    result["cache"]["status"] = json!("LOCAL_SOURCE_PROJECTION");
    result["provenance"] = json!({"kind":"local-source-projection/v1","evidenceHashes":evidence_hashes,
        "sourceAccountSha256":hash(crate::shared::diff::canonical_json(account)),"projectedResponseSha256":hash(raw),
        "originalProviderResponse":false,"networkCalls":0,
        "transcriptMeaning":"Compatibility transcript is a local projection of a retained normalized source account, not an original provider response."});
    Ok(result)
}
