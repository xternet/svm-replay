use super::*;

pub(in super::super) struct ArchivedTransaction<'a> {
    pub(in super::super) meta: &'a Value,
    pub(in super::super) keys: Vec<String>,
    pub(in super::super) writable: Vec<String>,
    pub(in super::super) signature: String,
}

pub(in super::super) fn pubkeys(value: &Value) -> Result<Vec<String>> {
    array(value)?
        .iter()
        .map(|value| {
            if value.is_string() {
                string(value).map(str::to_owned)
            } else {
                string(field(value, "pubkey")?).map(str::to_owned)
            }
        })
        .collect()
}

pub(in super::super) fn program_id(instruction: &Value, keys: &[String]) -> Result<String> {
    if let Some(Value::String(program)) = instruction.get("programId") {
        check(!program.is_empty(), "empty program id")?;
        return Ok(program.clone());
    }
    let index = usize_value(field(instruction, "programIdIndex")?)?;
    keys.get(index)
        .cloned()
        .ok_or_else(|| VerificationError("instruction program index out of range".into()))
}

pub(in super::super) fn validate_instructions(
    instructions: &[Value],
    keys: &[String],
) -> Result<Vec<String>> {
    let mut programs = Vec::new();
    for instruction in instructions {
        programs.push(program_id(instruction, keys)?);
        for index in array(field(instruction, "accounts")?)? {
            if let Some(key) = index.as_str() {
                check(
                    keys.iter().any(|value| value == key),
                    "undeclared instruction account",
                )?;
            } else {
                check(
                    usize_value(index)? < keys.len(),
                    "instruction account index out of range",
                )?;
            }
        }
    }
    Ok(programs)
}

pub(in super::super) fn archived_transaction(
    block: &Value,
    index: usize,
) -> Result<ArchivedTransaction<'_>> {
    let transactions = array(field(field(block, "result")?, "transactions")?)?;
    let envelope = transactions
        .get(index)
        .ok_or_else(|| VerificationError(format!("transaction {index} missing")))?;
    let transaction = field(envelope, "transaction")?;
    let message = field(transaction, "message")?;
    let meta = field(envelope, "meta")?;
    field(meta, "err")?;
    let signatures = strings(field(transaction, "signatures")?)?;
    let signature = signatures
        .first()
        .cloned()
        .ok_or_else(|| VerificationError("transaction signature missing".into()))?;
    let static_keys = pubkeys(field(message, "accountKeys")?)?;
    let mut writable_count = 0usize;
    let mut readonly_count = 0usize;
    if let Some(lookups) = message.get("addressTableLookups") {
        for lookup in array(lookups)? {
            writable_count = writable_count
                .checked_add(array(field(lookup, "writableIndexes")?)?.len())
                .ok_or_else(|| VerificationError("lookup count overflow".into()))?;
            readonly_count = readonly_count
                .checked_add(array(field(lookup, "readonlyIndexes")?)?.len())
                .ok_or_else(|| VerificationError("lookup count overflow".into()))?;
        }
    }
    let (loaded_writable, loaded_readonly) = match meta.get("loadedAddresses") {
        None | Some(Value::Null) => {
            check(
                writable_count == 0 && readonly_count == 0,
                "loaded addresses unavailable for declared lookup indices",
            )?;
            (Vec::new(), Vec::new())
        }
        Some(loaded) => (
            pubkeys(field(loaded, "writable")?)?,
            pubkeys(field(loaded, "readonly")?)?,
        ),
    };
    check(
        loaded_writable.len() == writable_count && loaded_readonly.len() == readonly_count,
        "loaded-address count differs from message lookup indices",
    )?;
    let mut keys = static_keys.clone();
    keys.extend(loaded_writable.iter().cloned());
    keys.extend(loaded_readonly);
    let instructions = array(field(message, "instructions")?)?;
    let outer_programs = validate_instructions(instructions, &keys)?;
    let mut groups = BTreeSet::new();
    let empty = json!([]);
    let inner = if crate::shared::instructions::is_unexecuted_load_failure(meta) {
        &empty
    } else {
        field(meta, "innerInstructions")?
    };
    for group in array(inner)? {
        let index = usize_value(field(group, "index")?)?;
        check(
            index < instructions.len(),
            "inner instruction group index malformed or out of range",
        )?;
        check(groups.insert(index), "duplicate inner instruction group")?;
        validate_instructions(array(field(group, "instructions")?)?, &keys)?;
    }
    let header = field(message, "header")?;
    let signatures = usize_value(field(header, "numRequiredSignatures")?)?;
    let readonly_signed = usize_value(field(header, "numReadonlySignedAccounts")?)?;
    let readonly_unsigned = usize_value(field(header, "numReadonlyUnsignedAccounts")?)?;
    check(
        signatures > 0 && signatures <= static_keys.len(),
        "message header missing/invalid fee payer",
    )?;
    check(
        readonly_signed <= signatures && readonly_unsigned <= static_keys.len() - signatures,
        "message header readonly counts malformed",
    )?;
    let demote = !keys
        .iter()
        .any(|key| key == "BPFLoaderUpgradeab1e11111111111111111111111");
    let writable = static_keys
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            *index < signatures - readonly_signed
                || (*index >= signatures && *index < static_keys.len() - readonly_unsigned)
        })
        .map(|(_, key)| key.clone())
        .chain(loaded_writable)
        .filter(|key| !demote || !outer_programs.contains(key))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(ArchivedTransaction {
        meta,
        keys,
        writable,
        signature,
    })
}
