use super::*;

pub(in super::super) fn inner_instructions(
    value: Option<&Value>,
    accounts: &[String],
    index: u64,
) -> Result<Instructions> {
    let groups = match value {
        None | Some(Value::Null) => {
            return Ok(Instructions {
                accounts: Vec::new(),
                programs: Vec::new(),
            })
        }
        Some(value) => array(
            value,
            &format!("transaction {index} inner instruction groups"),
        )?,
    };
    let mut referenced = Vec::new();
    let mut programs = Vec::new();
    for (i, group) in groups.iter().enumerate() {
        object(
            group,
            &format!("transaction {index} inner instruction group {i}"),
        )?;
        let group_index = integer(
            &group["index"],
            &format!("transaction {index} inner instruction group index"),
        )?;
        let parsed = instructions(
            &group["instructions"],
            accounts,
            index,
            &format!("inner instruction group {group_index} instruction"),
        )?;
        referenced.extend(parsed.accounts);
        programs.extend(parsed.programs);
    }
    Ok(Instructions {
        accounts: unique(referenced),
        programs: unique(programs),
    })
}

pub(in super::super) fn static_writable(keys: &[String], header: &Header) -> Result<Vec<String>> {
    if header.required > keys.len() as u64 {
        return Err(invalid(
            "message header has more signatures than account keys",
        ));
    }
    if header.readonly_signed > header.required {
        return Err(invalid(
            "message header has too many readonly signed accounts",
        ));
    }
    if header.readonly_unsigned > keys.len() as u64 - header.required {
        return Err(invalid(
            "message header has too many readonly unsigned accounts",
        ));
    }
    let signed_end = header.required - header.readonly_signed;
    let unsigned_end = keys.len() as u64 - header.readonly_unsigned;
    Ok(keys
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            (*i as u64) < signed_end
                || ((*i as u64) >= header.required && (*i as u64) < unsigned_end)
        })
        .map(|(_, key)| key.clone())
        .collect())
}
