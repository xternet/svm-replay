use super::*;

pub(in super::super) fn validate_sbpf(capture: &Value) -> Result<(), Error> {
    const INDEX_MAX: u64 = 9_007_199_254_740_991;
    check(
        unsigned(field(capture, "ordinal")?, true)? <= INDEX_MAX,
        "SBPF execution ordinal overflow",
    )?;
    let mut invocations = BTreeMap::new();
    for entry in array(field(capture, "invocations")?)? {
        let index = unsigned(field(entry, "invocation_index")?, true)?;
        let caller = unsigned(field(entry, "caller_index")?, true)?;
        let depth = unsigned(field(entry, "depth")?, true)?;
        check(
            index <= INDEX_MAX && depth > 0 && depth <= INDEX_MAX,
            "SBPF invocation/depth overflow",
        )?;
        check(
            if depth == 1 {
                caller == u16::MAX as u64 || caller == u64::MAX
            } else {
                caller <= INDEX_MAX && caller != index && caller != u16::MAX as u64
            },
            "SBPF caller identity differs from depth",
        )?;
        key(string(field(entry, "program_id")?)?)?;
        Digest::new(string(field(entry, "elf_sha256")?)?)?;
        let encoded = string(field(entry, "text_base64")?)?;
        let text = STANDARD
            .decode(encoded)
            .map_err(|error| invalid(format!("SBPF text: {error}")))?;
        check(
            !text.is_empty() && text.len() % 8 == 0 && STANDARD.encode(&text) == encoded,
            "SBPF executed text encoding/width differs",
        )?;
        check(
            invocations
                .insert(index, (caller, depth, text.len() as u64 / 8))
                .is_none(),
            "duplicate SBPF invocation",
        )?;
    }
    for (caller, depth, _) in invocations.values() {
        if let Some(parent) = invocations.get(caller) {
            check(
                parent.1.checked_add(1) == Some(*depth),
                "SBPF captured parent depth differs",
            )?;
        }
    }
    for row in array(field(capture, "trace")?)? {
        let row = array(row)?;
        check(row.len() == 2, "SBPF row width differs")?;
        let id = unsigned(&row[0], true)?;
        let entry = invocations
            .get(&id)
            .ok_or_else(|| invalid("SBPF row lacks captured executable"))?;
        let registers = array(&row[1])?;
        check(registers.len() == 12, "SBPF row requires twelve registers")?;
        for register in registers {
            unsigned(register, true)?;
        }
        check(
            unsigned(&registers[11], true)? < entry.2,
            "SBPF PC outside executed text",
        )?;
    }
    Ok(())
}

pub(in super::super) fn validate_memory_row(row: &Value) -> Result<(), Error> {
    super::super::super::contract::shape(
        row,
        &[
            "invocation_index",
            "pc",
            "address",
            "width",
            "operation",
            "succeeded",
            "value",
            "before",
            "after",
        ],
    )?;
    check(
        unsigned(field(row, "invocation_index")?, true)? <= 9_007_199_254_740_991,
        "memory invocation index overflow",
    )?;
    for name in ["pc", "address"] {
        let text = string(field(row, name)?)?;
        let hex = text
            .strip_prefix("0x")
            .ok_or_else(|| invalid("memory address/PC encoding"))?;
        let value = u64::from_str_radix(hex, 16)
            .map_err(|error| invalid(format!("memory address/PC: {error}")))?;
        check(
            text == format!("0x{value:x}"),
            "memory address/PC is not canonical",
        )?;
    }
    let width = unsigned(field(row, "width")?, true)?;
    check(
        [1, 2, 4, 8].contains(&width),
        "invalid memory operation width",
    )?;
    let operation = string(field(row, "operation")?)?;
    check(
        ["load", "store"].contains(&operation),
        "unknown memory operation",
    )?;
    let succeeded = field(row, "succeeded")?
        .as_bool()
        .ok_or_else(|| invalid("memory outcome missing"))?;
    for name in ["value", "before", "after"] {
        let bytes = field(row, name)?;
        if bytes.is_null() {
            continue;
        }
        raw_bytes(bytes)?;
        let bytes = array(bytes)?;
        check(
            bytes.len() == 8 && bytes[width as usize..].iter().all(|value| value == 0),
            "memory raw8/zero-padding differs",
        )?;
    }
    if operation == "load" {
        // Older hooks emit only the value; the newer hook also records identical
        // before/after snapshots for a successful read. Neither may imply a write.
        let no_snapshots = row["before"].is_null() && row["after"].is_null();
        let read_snapshots = succeeded
            && row["value"].is_array()
            && row["before"] == row["value"]
            && row["after"] == row["value"];
        check(
            (no_snapshots || read_snapshots) && (!succeeded || row["value"].is_array()),
            "load observation lacks bytes or has inconsistent read snapshots",
        )?;
    } else if succeeded {
        check(
            row["value"].is_array() && row["before"].is_array() && row["after"].is_array(),
            "successful store lacks raw byte snapshots",
        )?;
    }
    Ok(())
}
