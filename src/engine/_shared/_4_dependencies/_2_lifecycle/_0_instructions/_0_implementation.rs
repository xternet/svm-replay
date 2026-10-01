use super::*;

pub const LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

pub const LOADER_V4: &str = "LoaderV411111111111111111111111111111111111";

pub const ALT: &str = "AddressLookupTab1e1111111111111111111111111";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncludedTransaction {
    pub index: u64,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalLoaderContext {
    pub parent_slot: u64,
    pub accounts: Vec<Value>,
    pub included_transactions: Vec<IncludedTransaction>,
}

pub(in super::super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("LIFECYCLE_INPUT", message)
}

pub(in super::super::super) fn unsupported(
    code: &str,
    phase: &str,
    message: impl Into<String>,
) -> Error {
    Error::new(code, message).with_details(json!({"phase":phase}))
}

pub(in super::super) fn check(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid(message))
    }
}

pub(in super::super) fn object(value: &Value, label: &str) -> Result<()> {
    check(value.is_object(), format!("{label} malformed"))
}

pub(in super::super) fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| invalid(format!("{label} malformed")))
}

pub(in super::super) fn integer(value: &Value, label: &str) -> Result<usize> {
    let n = value
        .as_u64()
        .ok_or_else(|| invalid(format!("{label} malformed")))?;
    safe_integer(n, label)?;
    usize::try_from(n).map_err(|_| invalid(format!("{label} malformed")))
}

pub(in super::super) struct Instruction {
    pub(in super::super) index: usize,
    pub(in super::super) inner: bool,
    pub(in super::super) parent: usize,
    pub(in super::super) program: String,
    pub(in super::super) accounts: Vec<String>,
    pub(in super::super) data: Vec<u8>,
}

impl Instruction {
    pub(in super::super) fn order(&self) -> (usize, bool, usize) {
        (self.parent, self.inner, self.index)
    }
}

pub(in super::super) fn resolve(raw: &Value, index: u64) -> Result<Vec<Instruction>> {
    object(raw, &format!("transaction {index}"))?;
    object(&raw["transaction"], "transaction payload")?;
    let message = &raw["transaction"]["message"];
    object(message, "transaction message")?;
    let meta = &raw["meta"];
    object(meta, "transaction meta")?;
    object(&meta["loadedAddresses"], "loaded addresses")?;
    let mut accounts = Vec::new();
    for (v, label) in [
        (&message["accountKeys"], "static account keys"),
        (
            &meta["loadedAddresses"]["writable"],
            "loaded writable accounts",
        ),
        (
            &meta["loadedAddresses"]["readonly"],
            "loaded readonly accounts",
        ),
    ] {
        for key in array(v, label)? {
            let key = key
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| invalid(format!("{label} malformed")))?;
            accounts.push(key.to_owned());
        }
    }
    let resolve_group = |value: &Value, inner: bool, parent: usize| -> Result<Vec<Instruction>> {
        array(value, "transaction instructions")?
            .iter()
            .enumerate()
            .map(|(index, ix)| {
                object(ix, "transaction instruction")?;
                let program = match ix["programId"].as_str() {
                    Some(s) => s.to_owned(),
                    None => accounts
                        .get(integer(&ix["programIdIndex"], "program id index")?)
                        .ok_or_else(|| invalid("program id index out of range"))?
                        .clone(),
                };
                let referenced = array(&ix["accounts"], "instruction accounts")?
                    .iter()
                    .map(|value| {
                        if let Some(key) = value.as_str() {
                            check(
                                accounts.iter().any(|s| s == key),
                                "instruction account is undeclared",
                            )?;
                            Ok(key.to_owned())
                        } else {
                            Ok(accounts
                                .get(integer(value, "instruction account index")?)
                                .ok_or_else(|| invalid("instruction account index out of range"))?
                                .clone())
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                let data = ix["data"]
                    .as_str()
                    .ok_or_else(|| invalid("instruction data malformed"))?;
                let data = bs58::decode(data)
                    .into_vec()
                    .map_err(|_| invalid("base58 value malformed"))?;
                Ok(Instruction {
                    index,
                    inner,
                    parent: if inner { parent } else { index },
                    program,
                    accounts: referenced,
                    data,
                })
            })
            .collect()
    };
    let mut instructions = resolve_group(&message["instructions"], false, 0)?;
    // This pure prototype helper permits absent CPI recording. Source preparation
    // separately requires complete recording before it trusts the captured block.
    if !meta["innerInstructions"].is_null() {
        for group in array(&meta["innerInstructions"], "inner groups")? {
            object(group, "inner group")?;
            instructions.extend(resolve_group(
                &group["instructions"],
                true,
                integer(&group["index"], "inner parent instruction index")?,
            )?);
        }
    }
    Ok(instructions)
}
