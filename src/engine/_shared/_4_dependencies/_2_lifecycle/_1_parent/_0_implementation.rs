use super::*;

pub(in super::super) fn tag(data: &[u8]) -> Option<u32> {
    data.get(..4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub(in super::super) fn uint(data: &[u8], offset: usize) -> Result<u64> {
    let b = data
        .get(offset..offset + 8)
        .ok_or_else(|| invalid("loader u64 bytes missing"))?;
    Ok(u64::from_le_bytes(
        b.try_into()
            .map_err(|_| invalid("loader u64 bytes missing"))?,
    ))
}

pub(in super::super) fn exact<'a>(
    key: &str,
    accounts: &BTreeMap<&str, &'a Value>,
    slot: u64,
) -> Result<&'a Value> {
    let a = accounts
        .get(key)
        .copied()
        .ok_or_else(|| invalid(format!("{key}: exact loader parent input unavailable")))?;
    check(
        a["sourceSlot"].as_u64() == Some(slot)
            && matches!(a["presence"].as_str(), Some("present" | "absent")),
        format!("{key}: exact loader parent input unavailable"),
    )?;
    Ok(a)
}

pub(in super::super) fn parent(
    key: &str,
    executable: bool,
    accounts: &BTreeMap<&str, &Value>,
    slot: u64,
) -> Result<Vec<u8>> {
    let a = accounts.get(key).ok_or_else(|| {
        invalid(format!(
            "{key}: exact existing loader parent account unavailable or incompatible"
        ))
    })?;
    check(
        a["presence"] == "present"
            && a["sourceSlot"].as_u64() == Some(slot)
            && a["owner"] == LOADER
            && a["executable"] == executable
            && a["dataBase64"].is_string(),
        format!("{key}: exact existing loader parent account unavailable or incompatible"),
    )?;
    account_data(a).map_err(|_| invalid(format!("{key}: noncanonical parent data")))
}

pub(in super::super) fn authority(key: &str, data: &[u8], state: u32) -> Result<()> {
    let offset = if state == 1 { 4 } else { 12 };
    check(
        data.len() >= offset + 33 && tag(data) == Some(state),
        format!("{key}: invalid parent loader state layout"),
    )?;
    check(
        data[offset] <= 1,
        format!("{key}: invalid optional authority tag"),
    )
}

pub(in super::super) fn earlier<F>(
    raw: &Value,
    index: u64,
    current: &Instruction,
    context: &HistoricalLoaderContext,
    mut matches: F,
) -> Result<bool>
where
    F: FnMut(&Instruction) -> Result<bool>,
{
    for entry in &context.included_transactions {
        safe_integer(entry.index, "included loader transaction index")?;
        if entry.index > index || (entry.index == index && entry.raw != *raw) {
            continue;
        }
        for instruction in resolve(&entry.raw, entry.index)? {
            if instruction.program != LOADER
                || (entry.index == index && instruction.order() >= current.order())
            {
                continue;
            }
            if matches(&instruction)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(in super::super) fn deploy(instruction: &Instruction) -> bool {
    instruction.data.len() == 12
        && tag(&instruction.data) == Some(2)
        && instruction.accounts.len() >= 8
}

pub(in super::super) fn earlier_deploy(
    raw: &Value,
    index: u64,
    current: &Instruction,
    c: &HistoricalLoaderContext,
    program: &str,
    pd: &str,
) -> Result<()> {
    check(
        earlier(raw, index, current, c, |i| {
            Ok(deploy(i) && i.accounts[1] == pd && i.accounts[2] == program)
        })?,
        format!("{program}: absent loader state lacks an earlier included Deploy"),
    )
}

pub(in super::super) fn initialized(
    key: &str,
    raw: &Value,
    index: u64,
    current: &Instruction,
    c: &HistoricalLoaderContext,
) -> Result<bool> {
    earlier(raw, index, current, c, |i| {
        Ok(i.data.len() == 4
            && tag(&i.data) == Some(0)
            && i.accounts.len() >= 2
            && i.accounts[0] == key)
    })
}

pub(in super::super) fn buffer(
    key: &str,
    raw: &Value,
    index: u64,
    current: &Instruction,
    c: &HistoricalLoaderContext,
    accounts: &BTreeMap<&str, &Value>,
) -> Result<()> {
    if exact(key, accounts, c.parent_slot)?["presence"] == "present" {
        let data = parent(key, false, accounts, c.parent_slot)?;
        let state =
            tag(&data).ok_or_else(|| invalid(format!("{key}: parent Buffer state missing")))?;
        if state == 1 {
            return authority(key, &data, 1);
        }
        check(
            state == 0,
            format!("{key}: incompatible parent Buffer state"),
        )?;
    }
    check(
        initialized(key, raw, index, current, c)?,
        format!("{key}: absent/uninitialized Buffer lacks an earlier included InitializeBuffer"),
    )
}

pub(in super::super) fn mapping(
    program: &str,
    pd: &str,
    accounts: &BTreeMap<&str, &Value>,
    slot: u64,
    message: &str,
) -> Result<()> {
    let bytes = parent(program, true, accounts, slot)?;
    check(
        bytes.len() == 36
            && tag(&bytes) == Some(2)
            && bs58::encode(&bytes[4..36]).into_string() == pd,
        message,
    )
}
