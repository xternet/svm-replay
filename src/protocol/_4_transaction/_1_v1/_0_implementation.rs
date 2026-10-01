use super::*;

/// SIMD-0385 wire layout. Admission and feature activation remain runtime checks.
pub(in super::super) fn decode(bytes: &[u8]) -> Result<Value, Error> {
    let mut reader = Reader { bytes, position: 1 };
    let required = reader.byte()?;
    let readonly_signed = reader.byte()?;
    let readonly_unsigned = reader.byte()?;
    let mask = number(&mut reader, 4)?;
    if mask & !31 != 0 || matches!(mask & 3, 1 | 2) {
        return Err(invalid("invalid v1 transaction configuration mask"));
    }
    let blockhash = reader.address(32)?;
    let instruction_count = reader.byte()?;
    let key_count = reader.byte()?;
    if required == 0
        || required > 12
        || instruction_count > 64
        || key_count > 64
        || required > key_count
        || readonly_signed >= required
        || readonly_unsigned > key_count - required
    {
        return Err(invalid("malformed v1 message header"));
    }
    let mut keys = Vec::new();
    for _ in 0..key_count {
        keys.push(reader.address(32)?);
    }
    let mut config = json!({});
    for (bits, field, width) in [
        (3, "priorityFee", 8),
        (4, "computeUnitLimit", 4),
        (8, "loadedAccountsDataSizeLimit", 4),
        (16, "heapSize", 4),
    ] {
        config[field] = if mask & bits == bits {
            json!(number(&mut reader, width)?)
        } else {
            Value::Null
        };
    }
    let mut headers = Vec::new();
    for _ in 0..instruction_count {
        headers.push((reader.byte()?, reader.byte()?, number(&mut reader, 2)?));
    }
    let mut instructions = Vec::new();
    for (program, count, length) in headers {
        let accounts = reader.take(usize::from(count))?.to_vec();
        let data = reader.take(length as usize)?;
        if program == 0 || program >= key_count || accounts.iter().any(|&index| index >= key_count)
        {
            return Err(invalid("v1 instruction account index out of bounds"));
        }
        instructions.push(json!({"programIdIndex":program,"accounts":accounts,
            "data":bs58::encode(data).into_string()}));
    }
    let mut signatures = Vec::new();
    for _ in 0..required {
        signatures.push(reader.address(64)?);
    }
    if reader.position != bytes.len() {
        return Err(invalid("trailing v1 transaction bytes"));
    }
    Ok(
        json!({"version":1,"transaction":{"signatures":signatures,"message":{
        "header":{"numRequiredSignatures":required,"numReadonlySignedAccounts":readonly_signed,
            "numReadonlyUnsignedAccounts":readonly_unsigned},
        "accountKeys":keys,"recentBlockhash":blockhash,"instructions":instructions,
        "addressTableLookups":[],"transactionConfig":config}}}),
    )
}

fn number(reader: &mut Reader<'_>, width: usize) -> Result<u64, Error> {
    let mut bytes = [0; 8];
    bytes[..width].copy_from_slice(reader.take(width)?);
    Ok(u64::from_le_bytes(bytes))
}
