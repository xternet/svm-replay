use super::*;

pub const SLOT_HASHES: &str = "SysvarS1otHashes111111111111111111111111111";

pub const SLOT_HISTORY: &str = "SysvarS1otHistory11111111111111111111111111";

/// Only proven pre-execution failures may have absent execution recordings.
pub fn is_unexecuted_load_failure(meta: &Value) -> bool {
    matches!(
        meta["err"].as_str(),
        Some("MaxLoadedAccountsDataSizeExceeded" | "ProgramAccountNotFound")
    ) && meta.get("innerInstructions") == Some(&Value::Null)
        && meta.get("logMessages") == Some(&Value::Null)
        && meta["computeUnitsConsumed"].as_u64() == Some(0)
        && meta.get("returnData").is_none_or(Value::is_null)
}

#[derive(Clone, Debug)]
pub struct Instruction {
    pub index: usize,
    pub inner: bool,
    pub parent: Option<usize>,
    pub program: String,
    pub accounts: Vec<String>,
    pub data: Vec<u8>,
}

pub(in super::super) fn bad(message: impl Into<String>) -> Error {
    Error::new("HISTORICAL_INSTRUCTION", message)
}

pub(in super::super) fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>, Error> {
    value
        .as_array()
        .ok_or_else(|| bad(format!("{label} missing/malformed")))
}

pub(in super::super) fn index(value: &Value) -> Result<usize, Error> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| bad("invalid instruction index"))
}

pub(in super::super) fn keys(value: &Value) -> Result<Vec<String>, Error> {
    array(value, "account keys")?
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| bad("account key must be nonempty string"))
        })
        .collect()
}

pub fn resolve(raw: &Value) -> Result<Vec<Instruction>, Error> {
    let message = &raw["transaction"]["message"];
    let meta = &raw["meta"];
    let accounts = keys(&message["accountKeys"])?
        .into_iter()
        .chain(keys(&meta["loadedAddresses"]["writable"])?)
        .chain(keys(&meta["loadedAddresses"]["readonly"])?)
        .collect::<Vec<_>>();
    let outer = array(&message["instructions"], "outer instructions")?;
    let decode = |values: &[Value], parent: Option<usize>| -> Result<Vec<Instruction>, Error> {
        values
            .iter()
            .enumerate()
            .map(|(position, v)| {
                let key = |value: &Value| -> Result<String, Error> {
                    if let Some(text) = value.as_str() {
                        if !accounts.iter().any(|s| s == text) {
                            return Err(bad("instruction account is undeclared"));
                        }
                        Ok(text.into())
                    } else {
                        accounts
                            .get(index(value)?)
                            .cloned()
                            .ok_or_else(|| bad("account index outside message"))
                    }
                };
                let program = if let Some(program) = v["programId"].as_str() {
                    if program.is_empty() {
                        return Err(bad("empty program"));
                    }
                    program.to_owned()
                } else {
                    key(&v["programIdIndex"])?
                };
                let data = v["data"]
                    .as_str()
                    .ok_or_else(|| bad("instruction data missing"))?;
                Ok(Instruction {
                    index: position,
                    inner: parent.is_some(),
                    parent,
                    program,
                    accounts: array(&v["accounts"], "instruction accounts")?
                        .iter()
                        .map(key)
                        .collect::<Result<_, _>>()?,
                    data: bs58::decode(data)
                        .into_vec()
                        .map_err(|e| bad(format!("instruction base58:{e}")))?,
                })
            })
            .collect()
    };
    let mut instructions = decode(outer, None)?;
    let mut parents = std::collections::BTreeSet::new();
    // These top-level load errors occur before instruction execution. Keep outer
    // references for dependency analysis; accept null CPI only with zero-execution evidence.
    if is_unexecuted_load_failure(meta) {
        return Ok(instructions);
    }
    for group in array(&meta["innerInstructions"], "complete CPI recording")? {
        let parent = index(&group["index"])?;
        if parent >= outer.len() || !parents.insert(parent) {
            return Err(bad("CPI parent invalid/duplicate"));
        }
        instructions.extend(decode(
            array(&group["instructions"], "CPI instructions")?,
            Some(parent),
        )?);
    }
    Ok(instructions)
}

pub fn requirements(raw: &[Value], indices: &[u64], target: u64) -> Result<Value, Error> {
    let mut uses = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for &tx in indices {
        if !seen.insert(tx) {
            return Err(bad("execution indices overlap"));
        }
        for instruction in resolve(
            raw.get(tx as usize)
                .ok_or_else(|| bad("transaction index outside block"))?,
        )? {
            for (position, key) in instruction.accounts.iter().enumerate() {
                if ![RECENT_BLOCKHASHES, SLOT_HASHES, SLOT_HISTORY].contains(&key.as_str()) {
                    continue;
                }
                let hex = instruction
                    .data
                    .iter()
                    .map(|n| format!("{n:02x}"))
                    .collect::<String>();
                let mut entry = json!({"transactionIndex":tx,"instructionIndex":instruction.index,"inner":instruction.inner,
                    "programId":instruction.program,"sysvar":key,"accountPosition":position,"decodedDataHex":hex});
                if let Some(parent) = instruction.parent {
                    entry["parentInstructionIndex"] = json!(parent);
                }
                uses.push(entry);
            }
        }
    }
    classify(&uses, target)
}

pub fn classify(uses: &[Value], target: u64) -> Result<Value, Error> {
    let mut result = json!({});
    let mut exact = Vec::new();
    for (name, key) in [
        ("recentBlockhashes", RECENT_BLOCKHASHES),
        ("slotHashes", SLOT_HASHES),
        ("slotHistory", SLOT_HISTORY),
    ] {
        let records = uses
            .iter()
            .filter(|v| v["sysvar"] == key)
            .cloned()
            .collect::<Vec<_>>();
        let resolvable = !records.is_empty()
            && records.iter().all(|v| {
                key == RECENT_BLOCKHASHES
                    && v["inner"] == false
                    && v["programId"] == "11111111111111111111111111111111"
                    && v["accountPosition"] == 1
                    && v["decodedDataHex"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("04000000"))
            });
        let target_count = records
            .iter()
            .filter(|v| v["transactionIndex"].as_u64() == Some(target))
            .count();
        let required = !records.is_empty() && !resolvable;
        if required {
            exact.push(key);
        }
        result[name] = json!({"uses":records,"targetUses":target_count,"runtimeResolvable":resolvable,"exactAccountRequired":required});
    }
    result["exactAccountRequired"] = json!(exact);
    Ok(result)
}
