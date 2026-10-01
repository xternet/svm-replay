use super::*;

pub(in super::super) fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|x| x.is_ascii_digit() || (b'a'..=b'f').contains(&x))
}

pub(in super::super) fn read_blob(root: &Path, hash: &str, size: u64) -> Result<Vec<u8>> {
    if !valid_hash(hash) || size > MAX_BLOB_BYTES {
        return Err(StoreError::new(
            "CORRUPTION",
            "manifest contains invalid digest or size",
        ));
    }
    let path = root.join("blobs").join(hash);
    let file = File::open(path)
        .map_err(|e| StoreError::new("CORRUPTION", format!("manifest blob unavailable: {e}")))?;
    let mut bytes = Vec::new();
    file.take(MAX_BLOB_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != size || sha256(&bytes) != hash {
        return Err(StoreError::new(
            "CORRUPTION",
            "manifest blob size or SHA-256 mismatch",
        ));
    }
    Ok(bytes)
}

pub(in super::super) fn publish_blob(root: &Path, hash: &str, bytes: &[u8]) -> Result<()> {
    let dir = root.join("blobs");
    let path = dir.join(hash);
    if path.try_exists()? {
        read_blob(root, hash, bytes.len() as u64)?;
        return Ok(());
    }
    let mut staged = tempfile::Builder::new()
        .prefix(".pending-")
        .tempfile_in(&dir)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    #[cfg(windows)]
    super::_1_windows::publish(staged, &path)?;
    #[cfg(unix)]
    {
        staged
            .persist_noclobber(&path)
            .map_err(|e| StoreError::new("IO_ERROR", format!("publish immutable blob: {e}")))?;
        File::open(dir)?.sync_all()?;
    }
    Ok(())
}

pub(in super::super) fn evict(tx: &Transaction<'_>, max: u64) -> Result<Value> {
    let now = now_ms()?;
    let mut stmt = tx.prepare("SELECT hash,MAX(size),COUNT(*) FROM entries GROUP BY hash")?;
    let mut references: std::collections::BTreeMap<String, (u64, u64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?
        .collect::<std::result::Result<_, _>>()?;
    let mut remaining = 0_u64;
    for (size, _) in references.values() {
        remaining = remaining
            .checked_add(*size)
            .ok_or_else(|| StoreError::new("STORE_OVERFLOW", "blob byte total overflow"))?;
    }
    drop(stmt);
    let mut stmt = tx.prepare("SELECT namespace,key,hash FROM entries e WHERE NOT EXISTS(SELECT 1 FROM pins p WHERE p.namespace=e.namespace AND p.key=e.key) AND NOT EXISTS(SELECT 1 FROM leases l WHERE l.namespace=e.namespace AND l.key=e.key AND l.released=0 AND l.expires>?1) ORDER BY accessed,namespace,key")?;
    let candidates: Vec<(String, String, String)> = stmt
        .query_map([now as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<std::result::Result<_, _>>()?;
    drop(stmt);
    let mut deleted = 0_u64;
    for (ns, key, hash) in candidates {
        if remaining <= max {
            break;
        }
        tx.execute(
            "DELETE FROM entries WHERE namespace=?1 AND key=?2",
            params![ns, key],
        )?;
        let (size, count) = references
            .get_mut(&hash)
            .ok_or_else(|| StoreError::new("CORRUPTION", "missing blob reference accounting"))?;
        *count = count
            .checked_sub(1)
            .ok_or_else(|| StoreError::new("CORRUPTION", "blob reference underflow"))?;
        if *count == 0 {
            remaining = remaining
                .checked_sub(*size)
                .ok_or_else(|| StoreError::new("CORRUPTION", "blob byte total underflow"))?;
        }
        deleted += 1;
    }
    let protected: u64 = tx.query_row("SELECT COALESCE(SUM(size),0) FROM (SELECT hash,MAX(size) size FROM entries e WHERE EXISTS(SELECT 1 FROM pins p WHERE p.namespace=e.namespace AND p.key=e.key) OR EXISTS(SELECT 1 FROM leases l WHERE l.namespace=e.namespace AND l.key=e.key AND l.released=0 AND l.expires>?1) GROUP BY hash)", [now as i64], |row| row.get(0))?;
    Ok(
        json!({"version":1,"status":"OK","remainingBytes":remaining,"evictedEntries":deleted,"removedBlobs":0,"protectedBytes":protected,"limitSatisfied":remaining<=max}),
    )
}

pub(in super::super) fn collect_garbage(tx: &Transaction<'_>, root: &Path) -> Result<u64> {
    let mut stmt = tx.prepare("SELECT DISTINCT hash FROM entries")?;
    let kept: std::collections::BTreeSet<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    drop(stmt);
    let mut garbage = Vec::new();
    for item in fs::read_dir(root.join("blobs"))? {
        let item = item?;
        let name = item
            .file_name()
            .into_string()
            .map_err(|_| StoreError::new("CORRUPTION", "non-UTF8 blob directory entry"))?;
        if !item.file_type()?.is_file() {
            return Err(StoreError::new("CORRUPTION", "non-file in blob directory"));
        }
        if name.starts_with(".pending-") || (valid_hash(&name) && !kept.contains(&name)) {
            garbage.push(name);
        } else if !valid_hash(&name) {
            return Err(StoreError::new("CORRUPTION", "unexpected blob filename"));
        }
    }
    // This transaction excludes concurrent put while checking current references
    // and unlinking only files that no committed manifest can still require.
    for name in &garbage {
        fs::remove_file(root.join("blobs").join(name))?;
    }
    // Windows unlink has no directory-fsync equivalent here. A crash can leave
    // unreferenced garbage, which the next GC removes; no live blob is affected.
    #[cfg(unix)]
    if !garbage.is_empty() {
        File::open(root.join("blobs"))?.sync_all()?;
    }
    Ok(garbage.len() as u64)
}
