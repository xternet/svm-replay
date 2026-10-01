use super::*;

pub(super) fn inspect_directory(
    client: &mut DebugClient,
    name: &str,
    max_data_bytes: usize,
    directory: &Value,
) -> Result<Value, Error> {
    let accounts = directory["accounts"]
        .as_array()
        .ok_or_else(|| error("DEBUG_ACCOUNT", "validated account directory missing"))?;
    let account = accounts
        .iter()
        .find(|a| a["pubkey"] == name)
        .ok_or_else(|| {
            error(
                "DEBUG_ACCOUNT",
                format!(
                    "named account absent from bounded {} runtime directory: {name}",
                    directory["disposition"]
                ),
            )
        })?;
    let at = |field: &str| {
        account[field].as_u64().ok_or_else(|| {
            error(
                "DEBUG_ACCOUNT",
                format!("validated account {field} missing"),
            )
        })
    };
    let before = client.registers()?;
    let key = bs58::encode(client.memory(at("key")?, 32)?).into_string();
    require(
        key == name,
        "DEBUG_ACCOUNT",
        "guest-mutated key differs from runtime identity",
    )?;
    let owner = bs58::encode(client.memory(at("owner")?, 32)?).into_string();
    let mut raw = [0; 8];
    raw.copy_from_slice(&client.memory(at("lamports")?, 8)?);
    let lamports = u64::from_le_bytes(raw);
    raw.copy_from_slice(&client.memory(at("length")?, 8)?);
    let length = u64::from_le_bytes(raw);
    require(
        length <= at("maximumLength")?,
        "DEBUG_ACCOUNT",
        "live length exceeds runtime serialized allocation",
    )?;
    let count = (length as usize).min(max_data_bytes);
    let bytes = if count == 0 {
        Vec::new()
    } else {
        client.memory(at("data")?, count)?
    };
    let after = client.registers()?;
    require(
        client.state() == DebugState::Stopped && before == after,
        "DEBUG_ACCOUNT",
        "VM resumed or changed during inspection",
    )?;
    Ok(
        json!({"scope":"paused-vm-serialized-working-account","executionIndex":directory["executionIndex"],"invocationIndex":directory["identity"]["invocationIndex"],"program":directory["identity"]["program"],"elfSha256":directory["elfSha256"],"pc":format!("0x{:x}",before[11]),"pubkey":name,"owner":owner,"lamports":lamports.to_string(),"originalDataLength":account["originalLength"],"dataLength":length,"dataHex":bytes.iter().map(|b|format!("{b:02x}")).collect::<String>(),"disposition":if count as u64==length{"COMPLETE"}else{"TRUNCATED"},"runtimeAccountState":"unavailable: no paused TransactionContext account endpoint","committedState":"unavailable while paused: require finalized transaction output/journal"}),
    )
}
