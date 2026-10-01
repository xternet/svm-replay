use super::*;

pub(super) fn capture_bounds(captured: &Value, bounds: &ProducerBounds) -> Result<(), Error> {
    let captures = captured["captures"]
        .as_array()
        .ok_or_else(|| Error::new("DEBUG_PROTOCOL", "finalized execution captures missing"))?;
    let mut ordinals = BTreeSet::new();
    for execution in captures {
        let ordinal = execution["ordinal"]
            .as_u64()
            .ok_or_else(|| Error::new("DEBUG_PROTOCOL", "execution ordinal is not u64"))?;
        require(
            ordinal < u64::from(u16::MAX) && ordinals.insert(ordinal as u16),
            "DEBUG_PROTOCOL",
            "duplicate or oversized execution ordinal",
        )?;
        require(
            execution["execution_mode"] == "interpreter-debug"
                && execution["trace"]
                    .as_array()
                    .is_some_and(|rows| rows.len() as u64 <= bounds.register_rows)
                && execution["invocations"]
                    .as_array()
                    .is_some_and(|rows| rows.len() as u64 <= bounds.max_invocations),
            "DEBUG_LIMIT",
            "finalized mode/register/invocation bound differs",
        )?;
        match bounds.memory_rows {
            None => require(
                execution["memory"].is_null(),
                "DEBUG_PROTOCOL",
                "unrequested memory observations",
            )?,
            Some(maximum) => require(
                execution["memory"]["rows"]
                    .as_array()
                    .is_some_and(|rows| rows.len() as u64 <= maximum),
                "DEBUG_LIMIT",
                "memory producer bound differs",
            )?,
        }
    }
    Ok(())
}
