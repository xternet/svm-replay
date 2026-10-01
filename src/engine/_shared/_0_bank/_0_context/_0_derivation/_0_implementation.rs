use super::*;

pub(in super::super) const RECENT: &str = "SysvarRecentB1ockHashes11111111111111111111";

pub(in super::super) struct Instruction {
    pub(in super::super) program: String,
    pub(in super::super) accounts: Vec<String>,
    pub(in super::super) data: Vec<u8>,
    pub(in super::super) inner: bool,
}

pub(in super::super) fn resolve(transaction: &Value) -> Result<Vec<Instruction>> {
    let message = field(field(transaction, "transaction")?, "message")?;
    let meta = field(transaction, "meta")?;
    let loaded = field(meta, "loadedAddresses")?;
    let mut keys = Vec::new();
    for list in [
        field(message, "accountKeys")?,
        field(loaded, "writable")?,
        field(loaded, "readonly")?,
    ] {
        for value in array(list)? {
            let key = string(value)?;
            check(!key.is_empty(), "empty account key")?;
            keys.push(key.to_owned());
        }
    }
    let read = |values: &Value, inner: bool| -> Result<Vec<Instruction>> {
        array(values)?
            .iter()
            .map(|value| {
                let program = if let Some(Value::String(program)) = value.get("programId") {
                    program.clone()
                } else {
                    keys.get(integer(field(value, "programIdIndex")?)? as usize)
                        .cloned()
                        .ok_or_else(|| fail("program index out of range"))?
                };
                let mut accounts = Vec::new();
                for account in array(field(value, "accounts")?)? {
                    if let Some(key) = account.as_str() {
                        check(
                            keys.iter().any(|value| value == key),
                            "undeclared instruction account",
                        )?;
                        accounts.push(key.into());
                    } else {
                        accounts.push(
                            keys.get(integer(account)? as usize)
                                .cloned()
                                .ok_or_else(|| fail("account index out of range"))?,
                        );
                    }
                }
                let encoded = string(field(value, "data")?)?;
                let data = bs58::decode(encoded)
                    .into_vec()
                    .map_err(|e| fail(format!("instruction data: {e}")))?;
                Ok(Instruction {
                    program,
                    accounts,
                    data,
                    inner,
                })
            })
            .collect()
    };
    let mut instructions = read(field(message, "instructions")?, false)?;
    match meta.get("innerInstructions") {
        None | Some(Value::Null) => {}
        Some(groups) => {
            for group in array(groups)? {
                integer(field(group, "index")?)?;
                instructions.extend(read(field(group, "instructions")?, true)?);
            }
        }
    }
    Ok(instructions)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct FeeWitness {
    pub(in super::super) transaction_index: usize,
    pub(in super::super) fee: u64,
    pub(in super::super) signatures: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Lineage<'a> {
    pub(in super::super) source_hash: &'a str,
    pub(in super::super) target_slot: u64,
    pub(in super::super) parent_slot: u64,
    pub(in super::super) blockhash: &'a str,
    pub(in super::super) previous_blockhash: &'a str,
}
