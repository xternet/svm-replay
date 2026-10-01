use super::*;

pub(in super::super) struct Header {
    pub(in super::super) required: u64,
    pub(in super::super) readonly_signed: u64,
    pub(in super::super) readonly_unsigned: u64,
}

pub(in super::super) struct Instructions {
    pub(in super::super) accounts: Vec<String>,
    pub(in super::super) programs: Vec<String>,
}

pub(in super::super) fn object<'a>(
    value: &'a Value,
    label: &str,
) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} missing or malformed")))
}

pub(in super::super) fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| invalid(format!("{label} missing or malformed")))
}

pub(in super::super) fn integer(value: &Value, label: &str) -> Result<u64> {
    safe_integer(
        value
            .as_u64()
            .ok_or_else(|| invalid(format!("{label} is not a safe non-negative integer")))?,
        label,
    )
}

pub(in super::super) fn read_pubkey(value: &Value, label: &str) -> Result<String> {
    let value = if let Some(value) = value.as_str() {
        value
    } else {
        object(value, label)?
            .get("pubkey")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid(format!("{label} malformed")))?
    };
    pubkey(value, label)?;
    Ok(value.to_owned())
}

pub(in super::super) fn read_pubkeys(value: &Value, label: &str) -> Result<Vec<String>> {
    array(value, label)?
        .iter()
        .enumerate()
        .map(|(i, value)| read_pubkey(value, &format!("{label}[{i}]")))
        .collect()
}

pub(in super::super) fn header(value: &Value, index: u64) -> Result<Header> {
    object(value, &format!("transaction {index} message header"))?;
    Ok(Header {
        required: integer(
            &value["numRequiredSignatures"],
            &format!("transaction {index} numRequiredSignatures"),
        )?,
        readonly_signed: integer(
            &value["numReadonlySignedAccounts"],
            &format!("transaction {index} numReadonlySignedAccounts"),
        )?,
        readonly_unsigned: integer(
            &value["numReadonlyUnsignedAccounts"],
            &format!("transaction {index} numReadonlyUnsignedAccounts"),
        )?,
    })
}

pub(in super::super) fn loaded(
    value: Option<&Value>,
    index: u64,
) -> Result<(Vec<String>, Vec<String>)> {
    match value {
        None | Some(Value::Null) => Ok((Vec::new(), Vec::new())),
        Some(value) => {
            object(value, &format!("transaction {index} loaded addresses"))?;
            Ok((
                read_pubkeys(
                    &value["writable"],
                    &format!("transaction {index} loaded writable"),
                )?,
                read_pubkeys(
                    &value["readonly"],
                    &format!("transaction {index} loaded readonly"),
                )?,
            ))
        }
    }
}

pub(in super::super) fn address_tables(value: Option<&Value>, index: u64) -> Result<Vec<String>> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(value) => {
            let mut keys = Vec::new();
            for (i, lookup) in array(value, &format!("transaction {index} address table lookups"))?
                .iter()
                .enumerate()
            {
                object(
                    lookup,
                    &format!("transaction {index} address table lookup {i}"),
                )?;
                keys.push(read_pubkey(
                    &lookup["accountKey"],
                    &format!("transaction {index} address table lookup {i} accountKey"),
                )?);
            }
            Ok(unique(keys))
        }
    }
}

pub(in super::super) fn resolve_reference(
    value: &Value,
    accounts: &[String],
    label: &str,
) -> Result<String> {
    if let Some(value) = value.as_str() {
        if !accounts.iter().any(|account| account == value) {
            return Err(invalid(format!("{label} is undeclared")));
        }
        return Ok(value.to_owned());
    }
    let index = integer(value, label)?;
    if index >= accounts.len() as u64 {
        return Err(invalid(format!("{label} out of range")));
    }
    Ok(accounts[index as usize].clone())
}

pub(in super::super) fn instructions(
    value: &Value,
    accounts: &[String],
    index: u64,
    label: &str,
) -> Result<Instructions> {
    let mut referenced = Vec::new();
    let mut programs = Vec::new();
    for (i, instruction) in array(value, &format!("transaction {index} instructions"))?
        .iter()
        .enumerate()
    {
        object(instruction, &format!("transaction {index} {label} {i}"))?;
        let program = if let Some(program) = instruction["programId"].as_str() {
            program.to_owned()
        } else {
            let program_index = integer(
                &instruction["programIdIndex"],
                &format!("transaction {index} instruction {i} program index"),
            )?;
            if program_index >= accounts.len() as u64 {
                return Err(invalid(format!(
                    "transaction {index}: instruction {i} program index out of range"
                )));
            }
            accounts[program_index as usize].clone()
        };
        programs.push(program);
        for (position, account) in array(
            &instruction["accounts"],
            &format!("transaction {index} instruction {i} accounts"),
        )?
        .iter()
        .enumerate()
        {
            referenced.push(resolve_reference(
                account,
                accounts,
                &format!("transaction {index}: instruction {i} account {position}"),
            )?);
        }
    }
    Ok(Instructions {
        accounts: unique(referenced),
        programs: unique(programs),
    })
}
