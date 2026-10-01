use super::*;

pub(super) fn text_id(value: &str, max: usize, name: &str) -> Result<()> {
    if value.is_empty() || value.len() > max || value.contains('\0') {
        return Err(StoreError::new(
            "INVALID_REQUEST",
            format!("{name} must be nonempty, NUL-free and at most {max} bytes"),
        ));
    }
    Ok(())
}

pub(super) fn integer(value: u64) -> Result<()> {
    if value > MAX_INTEGER {
        return Err(StoreError::new(
            "INVALID_REQUEST",
            "integer exceeds exact JavaScript range",
        ));
    }
    Ok(())
}

pub(super) fn version(value: u64) -> Result<()> {
    if value != 1 {
        return Err(StoreError::new(
            "INVALID_REQUEST",
            "unsupported protocol version",
        ));
    }
    Ok(())
}

pub(super) fn now_ms() -> Result<u64> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| StoreError::new("CLOCK_ERROR", e.to_string()))?
        .as_millis();
    u64::try_from(ms).map_err(|e| StoreError::new("CLOCK_ERROR", e.to_string()))
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
