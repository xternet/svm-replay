use super::*;

pub(in super::super) struct Payload {
    pub(in super::super) bytes: Vec<u8>,
    pub(in super::super) events: u64,
    pub(in super::super) exhausted: bool,
}

impl Payload {
    pub(in super::super) fn append(
        &mut self,
        mut event: Value,
        request: &CaptureRequest,
    ) -> Result<(), Error> {
        if self.exhausted {
            return Ok(());
        }
        javascript_integers(&mut event);
        let encoded = serde_json::to_vec(&event).map_err(|error| invalid(error.to_string()))?;
        let separator = usize::from(self.events > 0);
        let total = self
            .bytes
            .len()
            .checked_add(encoded.len())
            .and_then(|size| size.checked_add(separator + 1))
            .ok_or_else(|| invalid("export byte count overflow"))?;
        if self.events == request.limits.max_events || total as u64 > request.limits.max_bytes {
            self.exhausted = true;
            return Ok(());
        }
        if separator > 0 {
            self.bytes.push(b',');
        }
        self.bytes.extend(encoded);
        self.events += 1;
        Ok(())
    }
}

// Convert from Rust's exact JSON numbers before JavaScript can round them.
// Keep ordinary counters/byte arrays numeric and preserve all original bits.
fn javascript_integers(value: &mut Value) {
    const MAX_SAFE: u64 = (1_u64 << 53) - 1;
    match value {
        Value::Number(number)
            if number.as_u64().is_some_and(|n| n > MAX_SAFE)
                || number.as_i64().is_some_and(|n| n < -(MAX_SAFE as i64)) =>
        {
            *value = Value::String(number.to_string());
        }
        Value::Array(values) => values.iter_mut().for_each(javascript_integers),
        Value::Object(values) => values.values_mut().for_each(javascript_integers),
        _ => {}
    }
}

/// Reviewed producers put mode either on the envelope or on each execution.
/// Missing evidence is an error; the requested mode never fills a producer field.
pub fn validate_execution_mode(output: &Value, expected: &str) -> Result<(), Error> {
    check(
        ["jit", "interpreter", "interpreter-debug"].contains(&expected),
        "unknown capture execution mode",
    )?;
    let captures = array(field(output, "captures")?)?;
    match output.get("execution_mode") {
        Some(mode) => check(mode == expected, "capture envelope mode differs")?,
        None => check(
            !captures.is_empty(),
            "capture has no execution mode evidence",
        )?,
    }
    for capture in captures {
        check(
            capture
                .get("execution_mode")
                .is_some_and(|mode| mode == expected),
            "captured execution mode missing or differs",
        )?;
    }
    Ok(())
}
