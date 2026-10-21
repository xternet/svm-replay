use super::*;

pub(in super::super) fn encode(archive: &Value) -> Result<Vec<u8>, Error> {
    let transaction = field(archive, "transaction")?;
    let message = field(transaction, "message")?;
    let signatures = signatures(transaction)?;
    let keys = keys(message)?;
    let instructions = array(field(message, "instructions")?)?;
    let mut bytes = vec![129];
    bytes.extend(header(message)?);
    let config = field(message, "transactionConfig")?;
    let mut mask = 0_u32;
    let mut parameters = Vec::new();
    for (bits, name, width) in [
        (3, "priorityFee", 8),
        (4, "computeUnitLimit", 4),
        (8, "loadedAccountsDataSizeLimit", 4),
        (16, "heapSize", 4),
    ] {
        let value = field(config, name)?;
        if !value.is_null() {
            mask |= bits;
            parameters.extend(uint(value, width)?);
        }
    }
    bytes.extend(mask.to_le_bytes());
    bytes.extend(data(field(message, "recentBlockhash")?, Some(32))?);
    bytes.push(
        u8::try_from(instructions.len()).map_err(|_| invalid("v1 instruction count overflow"))?,
    );
    bytes.push(u8::try_from(keys.len()).map_err(|_| invalid("v1 key count overflow"))?);
    for key in keys {
        bytes.extend(key);
    }
    bytes.extend(parameters);
    let mut payload = Vec::new();
    for ix in instructions {
        let accounts = array(field(ix, "accounts")?)?;
        let data = data(field(ix, "data")?, None)?;
        bytes.push(byte(field(ix, "programIdIndex")?)?);
        bytes.push(u8::try_from(accounts.len()).map_err(|_| invalid("v1 account count overflow"))?);
        bytes.extend(
            u16::try_from(data.len())
                .map_err(|_| invalid("v1 data size overflow"))?
                .to_le_bytes(),
        );
        for account in accounts {
            payload.push(byte(account)?);
        }
        payload.extend(data);
    }
    bytes.extend(payload);
    for signature in signatures {
        bytes.extend(signature);
    }
    Ok(bytes)
}
