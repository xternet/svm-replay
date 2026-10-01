use super::*;

pub(in super::super) fn account_bytes(account: &Value, owner: &str, slot: u64) -> Result<Vec<u8>> {
    check(
        account["presence"] == "present"
            && account["owner"] == owner
            && account["sourceSlot"] == slot,
        "historical account binding differs",
    )?;
    bytes(&account["dataBase64"])
}

pub(in super::super) fn rent_minimum(rent: &Value, parent: u64) -> Result<u64> {
    check(
        rent["pubkey"] == "SysvarRent111111111111111111111111111111111"
            && rent["executable"] == false,
        "wrong Rent image",
    )?;
    let data = account_bytes(rent, "Sysvar1111111111111111111111111111111111111", parent)?;
    check(data.len() == 17, "Rent encoding differs")?;
    let rate = u64::from_le_bytes(data[..8].try_into().map_err(|_| fail("Rent rate"))?);
    let threshold = f64::from_le_bytes(data[8..16].try_into().map_err(|_| fail("Rent threshold"))?);
    check(
        threshold.is_finite() && threshold > 0.0 && data[16] <= 100,
        "invalid Rent",
    )?;
    // Native Rent::minimum_balance includes the 128-byte account storage overhead.
    let base = rate
        .checked_mul(128 + 36)
        .ok_or_else(|| fail("Rent overflow"))?;
    let minimum = base as f64 * threshold;
    check(
        minimum >= 1.0 && minimum < u64::MAX as f64,
        "Rent minimum out of range",
    )?;
    Ok(minimum as u64)
}
