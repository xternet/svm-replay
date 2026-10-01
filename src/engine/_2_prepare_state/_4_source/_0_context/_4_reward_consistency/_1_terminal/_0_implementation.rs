use super::*;

pub(in super::super) struct TerminalContext<'a> {
    pub slot: u64,
    pub write_slot: u64,
    pub write_height: u64,
    pub height: u64,
    pub write_epoch: u64,
    pub epoch: u64,
    pub executor: &'a str,
    pub block_revenue_sharing: bool,
    pub rent_minimum: u64,
}

fn reject(message: &str) -> Error {
    Error::new(
        "UNSUPPORTED_BANK_INPUT",
        format!("EpochRewards terminal proof: {message}"),
    )
}

/// Pure transformation only. Caller authenticates cursor/block/runtime evidence.
/// A successful transformation is not by itself a qualified historical replay.
pub(in super::super) fn recover(
    account: &Value,
    write: &Value,
    ctx: &TerminalContext<'_>,
) -> Result<Value, Error> {
    let v42 = ctx.executor == "litesvm-v0.16.0-agave-4.2.1";
    if !(v42 || ctx.executor == "litesvm-v0.14.0-pr402-agave-4.1.2")
        || (!v42 && ctx.block_revenue_sharing)
    {
        return Err(reject("unreviewed runtime/feature branch"));
    }
    if ctx.write_slot > ctx.slot
        || ctx.write_epoch != ctx.epoch
        || account["sourceSlot"] != ctx.slot
        || write["sourceSlot"] != ctx.write_slot
    {
        return Err(reject("write boundary or epoch differs"));
    }
    for image in [account, write] {
        if image["pubkey"] != stake::EPOCH_REWARDS
            || image["presence"] != "present"
            || image["owner"] != "Sysvar1111111111111111111111111111111111111"
            || image["executable"] != false
        {
            return Err(reject("sysvar identity differs"));
        }
    }
    for field in ["dataBase64", "lamports", "rentEpoch", "owner", "executable"] {
        if account.get(field).is_none() || account[field] != write[field] {
            return Err(reject("point image differs from archived write"));
        }
    }
    let encoded = account["dataBase64"]
        .as_str()
        .ok_or_else(|| reject("missing data"))?;
    let mut bytes = STANDARD
        .decode(encoded)
        .map_err(|_| reject("invalid data"))?;
    if bytes.len() != 81 || bytes[80] != 1 {
        return Err(reject("expected active EpochRewards layout"));
    }
    let read = |offset| -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .map_err(|_| reject("integer encoding"))?,
        ))
    };
    let start = read(0)?;
    let partitions = read(8)?;
    let end = start
        .checked_add(partitions)
        .ok_or_else(|| reject("interval overflow"))?;
    if start == 0
        || partitions <= 1
        || ctx.write_height.checked_add(1) != Some(end)
        || ctx.height < ctx.write_height
        || read(72)? > read(64)?
    {
        return Err(reject("not a completed final-partition update"));
    }
    let amount = account["lamports"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| reject("lamports encoding"))?;
    if ctx.rent_minimum == 0 || amount < ctx.rent_minimum {
        return Err(reject("invalid native rent reserve"));
    }
    // Final update has already included both distributed and burned rewards.
    // Native deactivation changes active; 4.2 revenue-sharing also burns surplus.
    bytes[80] = 0;
    let mut result = account.clone();
    result["dataBase64"] = json!(STANDARD.encode(bytes));
    if ctx.block_revenue_sharing {
        result["lamports"] = json!(ctx.rent_minimum.to_string());
    }
    Ok(result)
}
