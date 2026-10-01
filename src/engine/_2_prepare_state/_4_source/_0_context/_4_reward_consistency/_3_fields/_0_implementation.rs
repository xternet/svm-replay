use super::*;

pub(in super::super) fn epoch(clock: &Value, slot: u64) -> Result<u64, Error> {
    sysvars::validate_target_clock(clock, slot)?;
    let bytes = STANDARD
        .decode(
            clock["dataBase64"]
                .as_str()
                .ok_or_else(|| invalid("Clock data"))?,
        )
        .map_err(|_| invalid("Clock data"))?;
    Ok(u64::from_le_bytes(
        bytes[16..24]
            .try_into()
            .map_err(|_| invalid("Clock epoch"))?,
    ))
}

pub(in super::super) fn rent_minimum(rent: &Value, slot: u64) -> Result<u64, Error> {
    sysvars::bind_exact_generic_sysvar(rent, slot)?;
    if rent["pubkey"] != RENT {
        return Err(invalid("terminal proof Rent identity"));
    }
    let bytes = STANDARD
        .decode(
            rent["dataBase64"]
                .as_str()
                .ok_or_else(|| invalid("Rent data"))?,
        )
        .map_err(|_| invalid("Rent data"))?;
    if bytes.len() != 17 {
        return Err(invalid("Rent size"));
    }
    let rate = u64::from_le_bytes(bytes[..8].try_into().map_err(|_| invalid("Rent rate"))?);
    let threshold = f64::from_le_bytes(
        bytes[8..16]
            .try_into()
            .map_err(|_| invalid("Rent threshold"))?,
    );
    if !threshold.is_finite() || threshold <= 0.0 || bytes[16] > 100 {
        return Err(invalid("Rent values"));
    }
    // Same arithmetic as native Rent::minimum_balance, for the 81-byte sysvar.
    let base = rate
        .checked_mul(128 + 81)
        .ok_or_else(|| invalid("Rent overflow"))?;
    let minimum = base as f64 * threshold;
    if minimum < 1.0 || minimum >= u64::MAX as f64 {
        return Err(invalid("Rent range"));
    }
    Ok(minimum as u64)
}
