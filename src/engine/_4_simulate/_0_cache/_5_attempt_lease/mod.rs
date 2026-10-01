use super::*;

pub(super) fn attempt_lease(owner: &str) -> Result<Value, Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::new("CACHE_CLOCK", error.to_string()))?
        .as_millis();
    let expires = u64::try_from(now)
        .ok()
        .and_then(|value| value.checked_add(600_000))
        .ok_or_else(|| Error::new("CACHE_CLOCK", "lease expiry overflow"))?;
    let lease = json!({"owner":owner,"expiresAtMs":expires});
    Ok(lease)
}
