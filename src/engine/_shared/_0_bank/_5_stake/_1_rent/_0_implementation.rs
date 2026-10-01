use super::*;

pub(in super::super) struct RentAdjustment {
    pub(in super::super) minimum: u64,
    pub(in super::super) rewarded_epoch: u64,
    pub(in super::super) proof: Value,
}

pub(in super::super) fn rent_phase(context: Option<&Value>, slot: u64) -> Result<RentAdjustment> {
    let context = context.ok_or_else(|| fail("exact target Clock/Rent required"))?;
    let clock_account = field(context, "clock")?;
    let rent_account = field(context, "rent")?;
    let clock = phase_bytes(clock_account, sysvars::CLOCK_SYSVAR, slot, 40)?;
    let rent = phase_bytes(
        rent_account,
        "SysvarRent111111111111111111111111111111111",
        slot,
        17,
    )?;
    check(read_u64(&clock, 0)? == slot, "target Clock slot")?;
    let rate = read_u64(&rent, 0)?;
    let threshold = f64::from_le_bytes(
        rent[8..16]
            .try_into()
            .map_err(|e| fail(format!("Rent threshold: {e}")))?,
    );
    let product = 328u128 * u128::from(rate); // StakeStateV2 allocation + canonical Rent overhead.
    let minimum = if threshold == 1.0 || threshold == 2.0 {
        let multiplier = threshold as u64;
        check(
            rate <= u64::MAX / (10 * 1024 * 1024 + 128) / multiplier,
            "canonical Rent maximum rate",
        )?;
        u64::try_from(product * u128::from(multiplier))
            .map_err(|e| fail(format!("Rent minimum: {e}")))?
    } else {
        check(
            product <= u128::from(u64::MAX),
            "unproven overflowing Rent product",
        )?;
        // Rust's saturating float-to-u64 cast matches canonical Rent's NaN,
        // negative, infinity and truncation behavior after exact integer product.
        ((product as u64) as f64 * threshold) as u64
    };
    let epoch = read_u64(&clock, 16)?;
    Ok(RentAdjustment {
        minimum,
        rewarded_epoch: epoch.saturating_sub(1),
        proof: json!({"schema":"agave-4.2.1-rent-delegation-adjustment/v1",
        "clockAccountSha256":hash(canonical_json(clock_account)),"rentAccountSha256":hash(canonical_json(rent_account)),
        "minimumBalance":minimum.to_string(),"epoch":epoch.to_string()}),
    })
}

pub(in super::super) fn executed_failure(error: &Value, instructions: usize) -> bool {
    let Some(error) = error.as_object() else {
        return false;
    };
    if error.len() != 1 {
        return false;
    }
    let Some(value) = error.get("InstructionError").and_then(Value::as_array) else {
        return false;
    };
    if value.len() != 2 {
        return false;
    }
    let Some(index) = value[0].as_u64() else {
        return false;
    };
    if index >= instructions as u64 {
        return false;
    }
    if value[1].as_str().is_some_and(|text| !text.is_empty()) {
        return true;
    }
    value[1].as_object().is_some_and(|reason| {
        reason.len() == 1
            && reason
                .get("Custom")
                .and_then(Value::as_u64)
                .is_some_and(|value| value <= u32::MAX as u64)
    })
}
