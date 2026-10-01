use super::*;

pub(in super::super) const ALT: &str = "AddressLookupTab1e1111111111111111111111111";

pub(in super::super) const SYSTEM: &str = "11111111111111111111111111111111";

#[derive(Debug, Clone)]
pub struct RequestedBoundaryEvidence {
    pub target_slot: u64,
    pub raw_transactions: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestedResolution {
    pub raw: Value,
    pub summary: SemanticTransaction,
}

pub(in super::super) fn unsupported(message: impl Into<String>) -> Error {
    Error::new("UNSUPPORTED_ALT_LIFECYCLE", message).with_details(json!({"phase":"PREFLIGHT"}))
}

pub(in super::super) fn object(value: &Value, label: &str) -> Result<()> {
    if !value.is_object() {
        return Err(invalid(format!("{label} malformed")));
    }
    Ok(())
}

pub(in super::super) fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| invalid(format!("{label} malformed")))
}

pub(in super::super) fn string<'a>(value: &'a Value, label: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| invalid(format!("{label} malformed")))
}

pub(in super::super) fn index(value: &Value, label: &str) -> Result<u64> {
    safe_integer(
        value
            .as_u64()
            .ok_or_else(|| invalid(format!("{label} malformed")))?,
        label,
    )
}

pub(in super::super) fn string_keys(value: &Value, label: &str) -> Result<Vec<String>> {
    array(value, label)?
        .iter()
        .map(|value| {
            let key = string(value, label)?;
            if key.is_empty() {
                return Err(invalid(format!("{label} malformed")));
            }
            Ok(key.to_owned())
        })
        .collect()
}

pub(in super::super) fn pubkeys(value: &Value, label: &str) -> Result<Vec<String>> {
    array(value, label)?
        .iter()
        .map(|value| {
            let key = if value.is_string() {
                string(value, label)?
            } else {
                object(value, label)?;
                string(&value["pubkey"], label)?
            };
            if key.is_empty() {
                return Err(invalid(format!("{label} malformed")));
            }
            Ok(key.to_owned())
        })
        .collect()
}

pub(in super::super) struct Instruction {
    pub(in super::super) parent: u64,
    pub(in super::super) inner: bool,
    pub(in super::super) index: usize,
    pub(in super::super) program: String,
    pub(in super::super) accounts: Vec<String>,
    pub(in super::super) data: Vec<u8>,
}

pub(in super::super) fn instructions(
    value: &Value,
    accounts: &[String],
    inner: bool,
    parent: u64,
) -> Result<Vec<Instruction>> {
    let mut resolved = Vec::new();
    for (position, instruction) in array(value, "instructions")?.iter().enumerate() {
        object(instruction, "instruction")?;
        let program = if instruction["programId"].is_string() {
            string(&instruction["programId"], "program id")?.to_owned()
        } else {
            let i = index(&instruction["programIdIndex"], "program id index")?;
            accounts
                .get(usize::try_from(i).map_err(|_| invalid("program id index out of range"))?)
                .ok_or_else(|| invalid("program id index out of range"))?
                .clone()
        };
        let mut referenced = Vec::new();
        for reference in array(&instruction["accounts"], "instruction accounts")? {
            let key = if let Some(key) = reference.as_str() {
                if !accounts.iter().any(|account| account == key) {
                    return Err(invalid("instruction account is undeclared"));
                }
                key.to_owned()
            } else {
                let i = index(reference, "instruction account index")?;
                accounts
                    .get(
                        usize::try_from(i)
                            .map_err(|_| invalid("instruction account index out of range"))?,
                    )
                    .ok_or_else(|| invalid("instruction account index out of range"))?
                    .clone()
            };
            referenced.push(key);
        }
        let data = bs58::decode(string(&instruction["data"], "instruction data")?)
            .into_vec()
            .map_err(|e| invalid(format!("instruction base58 malformed:{e}")))?;
        resolved.push(Instruction {
            parent: if inner { parent } else { position as u64 },
            inner,
            index: position,
            program,
            accounts: referenced,
            data,
        });
    }
    Ok(resolved)
}

pub(in super::super) fn recorded_instructions(
    raw: &Value,
    succeeded: bool,
) -> Result<Vec<Instruction>> {
    object(raw, "ALT writer")?;
    object(&raw["transaction"], "ALT writer transaction")?;
    let message = &raw["transaction"]["message"];
    object(message, "ALT writer message")?;
    let meta = &raw["meta"];
    object(meta, "ALT writer meta")?;
    let outer_count = array(&message["instructions"], "ALT writer instructions")?.len();
    let groups = array(
        &meta["innerInstructions"],
        "ALT writer inner instructions unavailable or malformed",
    )?;
    let mut seen = BTreeSet::new();
    for group in groups {
        object(group, "ALT writer inner instruction group")?;
        let parent = index(&group["index"], "ALT writer inner group index")?;
        if parent >= outer_count as u64 {
            return Err(invalid(
                "ALT writer inner instruction group index out of range",
            ));
        }
        if !seen.insert(parent) {
            return Err(invalid("ALT writer duplicate inner instruction group"));
        }
        array(&group["instructions"], "ALT writer inner instructions")?;
    }
    // Recorded CPI shape is required even after failure, but ALT mutations rolled back.
    if !succeeded {
        return Ok(Vec::new());
    }
    object(&meta["loadedAddresses"], "ALT writer loaded addresses")?;
    let accounts: Vec<_> = string_keys(&message["accountKeys"], "static account keys")?
        .into_iter()
        .chain(string_keys(
            &meta["loadedAddresses"]["writable"],
            "loaded writable accounts",
        )?)
        .chain(string_keys(
            &meta["loadedAddresses"]["readonly"],
            "loaded readonly accounts",
        )?)
        .collect();
    let mut resolved = instructions(&message["instructions"], &accounts, false, 0)?;
    for group in groups {
        resolved.extend(instructions(
            &group["instructions"],
            &accounts,
            true,
            index(&group["index"], "inner parent instruction index")?,
        )?);
    }
    resolved.sort_by_key(|instruction| (instruction.parent, instruction.inner, instruction.index));
    Ok(resolved)
}
