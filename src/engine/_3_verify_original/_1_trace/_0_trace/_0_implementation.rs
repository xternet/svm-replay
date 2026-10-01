use super::*;

pub(in super::super) fn fields(value: &Value, expected: &[&str]) -> Result<()> {
    let value = object(value)?;
    check(
        value.len() == expected.len() && expected.iter().all(|key| value.contains_key(*key)),
        "instruction trace unexpected/missing fields",
    )
}

pub(in super::super) fn index(value: &Value, maximum: u64) -> Result<usize> {
    let index = integer(value)?;
    check(index <= maximum, "instruction trace invalid index")?;
    usize::try_from(index)
        .map_err(|error| VerificationError(format!("instruction trace index: {error}")))
}

pub fn assert_runtime_instruction_trace(trace: &Value) -> Result<()> {
    fields(trace, &["schema", "source", "coverage", "groups"])?;
    check(
        field(trace, "schema")? == "svm-inner-instructions/v1"
            && field(trace, "source")? == "runtime"
            && field(trace, "coverage")? == "recorded-invocations",
        "instruction trace schema/source/coverage",
    )?;
    let mut previous = None;
    for group in array(field(trace, "groups")?)? {
        fields(group, &["index", "instructions"])?;
        let outer = index(field(group, "index")?, 9_007_199_254_740_991)?;
        if let Some(previous) = previous {
            check(
                outer > previous,
                "instruction trace unordered/duplicate group",
            )?;
        }
        previous = Some(outer);
        let entries = array(field(group, "instructions")?)?;
        check(!entries.is_empty(), "instruction trace empty group")?;
        let mut position: Vec<usize> = Vec::new();
        for entry in entries {
            fields(
                entry,
                &[
                    "path",
                    "stackHeight",
                    "programIdIndex",
                    "accounts",
                    "dataBase64",
                ],
            )?;
            let height = index(field(entry, "stackHeight")?, 255)?;
            check(height >= 2, "instruction trace incoherent depth")?;
            let depth = height - 1;
            check(
                depth <= position.len() + 1,
                "instruction trace incoherent depth",
            )?;
            if depth > position.len() {
                position.push(0);
            } else {
                position.truncate(depth);
                position[depth - 1] += 1;
            }
            let mut path = vec![outer];
            path.extend(&position);
            check(
                field(entry, "path")? == &json!(path),
                "instruction trace path does not match runtime depth/order",
            )?;
            index(field(entry, "programIdIndex")?, 255)?;
            for account in array(field(entry, "accounts")?)? {
                index(account, 255)?;
            }
            base64_bytes(field(entry, "dataBase64")?)?;
        }
    }
    Ok(())
}

pub fn verify_runtime_instruction_trace(trace: &Value, recorded: &Value) -> Result<Value> {
    assert_runtime_instruction_trace(trace)?;
    let archived = array(recorded)?;
    let mut previous = None;
    let mut nonempty = Vec::new();
    for group in archived {
        let outer = index(field(group, "index")?, 9_007_199_254_740_991)?;
        if let Some(previous) = previous {
            check(
                outer > previous,
                "instruction trace unordered/duplicate archived group",
            )?;
        }
        previous = Some(outer);
        if !array(field(group, "instructions")?)?.is_empty() {
            nonempty.push(group);
        }
    }
    let groups = array(field(trace, "groups")?)?;
    check(
        groups.len() == nonempty.len(),
        "instruction trace group count mismatch",
    )?;
    let mut instructions = 0;
    let mut verified = 0;
    let mut unavailable = 0;
    for (group, actual) in nonempty.into_iter().zip(groups) {
        check(
            field(group, "index")? == field(actual, "index")?,
            "instruction trace outer index mismatch",
        )?;
        let archived_entries = array(field(group, "instructions")?)?;
        let actual_entries = array(field(actual, "instructions")?)?;
        check(
            archived_entries.len() == actual_entries.len(),
            "instruction trace instruction count mismatch",
        )?;
        for (expected, entry) in archived_entries.iter().zip(actual_entries) {
            check(
                index(field(expected, "programIdIndex")?, 255)?
                    == index(field(entry, "programIdIndex")?, 255)?,
                "instruction trace program index mismatch",
            )?;
            for account in array(field(expected, "accounts")?)? {
                index(account, 255)?;
            }
            check(
                field(expected, "accounts")? == field(entry, "accounts")?,
                "instruction trace positional accounts mismatch",
            )?;
            let data = field(expected, "data")?
                .as_str()
                .ok_or_else(|| VerificationError("archived instruction bytes missing".into()))?;
            let bytes = bs58::decode(data).into_vec().map_err(|error| {
                VerificationError(format!("archived instruction bytes: {error}"))
            })?;
            check(
                bs58::encode(&bytes).into_string() == data
                    && bytes == base64_bytes(field(entry, "dataBase64")?)?,
                "instruction trace instruction bytes mismatch",
            )?;
            match expected.get("stackHeight") {
                None | Some(Value::Null) => unavailable += 1,
                Some(height) => {
                    check(
                        index(height, 255)? == index(field(entry, "stackHeight")?, 255)?,
                        "instruction trace stack height mismatch",
                    )?;
                    verified += 1;
                }
            }
            instructions += 1;
        }
    }
    Ok(json!({"status":"MATCH", "instructions":instructions,
        "stackHeightsVerified":verified, "stackHeightsUnavailable":unavailable}))
}
