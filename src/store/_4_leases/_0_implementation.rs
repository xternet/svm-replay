use super::*;

pub(in super::super) fn validate_owned(key: &str, owner: &str) -> Result<()> {
    text_id(key, 8192, "key")?;
    text_id(owner, 256, "owner")
}

pub(in super::super) fn next_seq(tx: &Transaction<'_>) -> Result<i64> {
    let old: i64 = tx.query_row("SELECT seq FROM meta WHERE id=1", [], |row| row.get(0))?;
    let next = old
        .checked_add(1)
        .ok_or_else(|| StoreError::new("STORE_OVERFLOW", "access sequence exhausted"))?;
    tx.execute("UPDATE meta SET seq=?1 WHERE id=1", [next])?;
    Ok(next)
}

pub(in super::super) fn entry(
    tx: &Transaction<'_>,
    ns: &str,
    key: &str,
) -> Result<Option<(String, u64)>> {
    Ok(tx
        .query_row(
            "SELECT hash,size FROM entries WHERE namespace=?1 AND key=?2",
            params![ns, key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

pub(in super::super) fn require_entry(tx: &Transaction<'_>, ns: &str, key: &str) -> Result<()> {
    if entry(tx, ns, key)?.is_none() {
        return Err(StoreError::new("NOT_FOUND", "entry does not exist"));
    }
    Ok(())
}

pub(in super::super) fn validate_lease(lease: &Lease) -> Result<()> {
    text_id(&lease.owner, 256, "owner")?;
    integer(lease.expires_at_ms)?;
    if lease.expires_at_ms <= now_ms()? {
        return Err(StoreError::new(
            "LEASE_EXPIRED",
            "lease deadline has elapsed; operation cannot use it",
        ));
    }
    Ok(())
}

pub(in super::super) fn acquire_lease(
    tx: &Transaction<'_>,
    ns: &str,
    key: &str,
    lease: &Lease,
) -> Result<()> {
    validate_lease(lease)?;
    let previous: Option<(u64, bool)> = tx
        .query_row(
            "SELECT expires,released FROM leases WHERE namespace=?1 AND key=?2 AND owner=?3",
            params![ns, key, lease.owner],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((expires, released)) = previous {
        if released || expires != lease.expires_at_ms {
            return Err(StoreError::new(
                "LEASE_CONFLICT",
                "lease identity cannot be renewed or reopened after release",
            ));
        }
    } else {
        tx.execute(
            "INSERT INTO leases VALUES(?1,?2,?3,?4,0)",
            params![ns, key, lease.owner, lease.expires_at_ms as i64],
        )?;
    }
    Ok(())
}

pub(in super::super) fn lease_value(lease: Option<&Lease>) -> Value {
    match lease {
        Some(x) => json!({"owner":x.owner,"expiresAtMs":x.expires_at_ms}),
        None => Value::Null,
    }
}
