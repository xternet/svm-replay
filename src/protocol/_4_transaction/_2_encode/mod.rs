use super::_0_decode::invalid;
use super::*;
mod _0_fields;
use _0_fields::{array, byte, data, field, header, keys, shortvec, signatures, uint};
mod _1_legacy;
mod _2_v1;
pub fn encode(archive: &Value) -> Result<String, Error> {
    let bytes = match archive.get("version") {
        None => _1_legacy::encode(archive, false)?,
        Some(value) if value == "legacy" => _1_legacy::encode(archive, false)?,
        Some(value) if value == &json!(0) => _1_legacy::encode(archive, true)?,
        Some(value) if value == &json!(1) => _2_v1::encode(archive)?,
        _ => {
            return Err(Error::new(
                "UNSUPPORTED_TRANSACTION_VERSION",
                "only legacy, v0 and v1 wire formats are reviewed",
            ))
        }
    };
    let encoded = STANDARD.encode(bytes);
    super::assert_historical(&encoded, archive)?;
    Ok(encoded)
}
#[cfg(test)]
mod tests;
