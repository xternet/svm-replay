use super::*;

impl Store {
    pub fn execute(&mut self, input: Value) -> Result<Value> {
        let command: Command = serde_json::from_value(input)
            .map_err(|e| StoreError::new("INVALID_REQUEST", e.to_string()))?;
        let root = self.root.clone();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = match command {
            Command::Put {
                version: v,
                namespace,
                key,
                data_base64,
                lease,
            } => {
                version(v)?;
                text_id(&key, 8192, "key")?;
                if data_base64.len() as u64 > MAX_BLOB_BYTES.div_ceil(3) * 4 {
                    return Err(StoreError::new("INPUT_TOO_LARGE", "blob exceeds 256 MiB"));
                }
                let data = STANDARD
                    .decode(data_base64)
                    .map_err(|e| StoreError::new("INVALID_REQUEST", e.to_string()))?;
                if data.len() as u64 > MAX_BLOB_BYTES {
                    return Err(StoreError::new("INPUT_TOO_LARGE", "blob exceeds 256 MiB"));
                }
                let hash = sha256(&data);
                let ns = namespace.as_str();
                if let Some((existing, size)) = entry(&tx, ns, &key)? {
                    if existing != hash {
                        return Err(StoreError::new(
                            "CONFLICT",
                            "immutable key already binds different bytes",
                        ));
                    }
                    read_blob(&root, &existing, size)?;
                } else {
                    publish_blob(&root, &hash, &data)?;
                    #[cfg(test)]
                    crash_point("blob_durable");
                    let seq = next_seq(&tx)?;
                    tx.execute(
                        "INSERT INTO entries VALUES(?1,?2,?3,?4,?5)",
                        params![ns, key, hash, data.len() as i64, seq],
                    )?;
                    #[cfg(test)]
                    crash_point("manifest_uncommitted");
                }
                if let Some(lease) = &lease {
                    acquire_lease(&tx, ns, &key, lease)?;
                }
                json!({"version":1,"status":"STORED","sha256":hash,"sizeBytes":data.len(),"lease":lease_value(lease.as_ref())})
            }
            Command::Get {
                version: v,
                namespace,
                key,
                lease,
            } => {
                version(v)?;
                text_id(&key, 8192, "key")?;
                let ns = namespace.as_str();
                if let Some((hash, size)) = entry(&tx, ns, &key)? {
                    if let Some(lease) = &lease {
                        acquire_lease(&tx, ns, &key, lease)?;
                    }
                    let bytes = read_blob(&root, &hash, size)?;
                    if let Some(lease) = &lease {
                        validate_lease(lease)?;
                    }
                    let seq = next_seq(&tx)?;
                    tx.execute(
                        "UPDATE entries SET accessed=?1 WHERE namespace=?2 AND key=?3",
                        params![seq, ns, key],
                    )?;
                    json!({"version":1,"status":"HIT","sha256":hash,"sizeBytes":size,"dataBase64":STANDARD.encode(bytes),"lease":lease_value(lease.as_ref())})
                } else {
                    if let Some(lease) = &lease {
                        validate_lease(lease)?;
                    }
                    json!({"version":1,"status":"MISS"})
                }
            }
            Command::Pin {
                version: v,
                namespace,
                key,
                owner,
            } => {
                version(v)?;
                validate_owned(&key, &owner)?;
                require_entry(&tx, namespace.as_str(), &key)?;
                tx.execute(
                    "INSERT OR IGNORE INTO pins VALUES(?1,?2,?3)",
                    params![namespace.as_str(), key, owner],
                )?;
                json!({"version":1,"status":"OK"})
            }
            Command::Unpin {
                version: v,
                namespace,
                key,
                owner,
            } => {
                version(v)?;
                validate_owned(&key, &owner)?;
                if tx.execute(
                    "DELETE FROM pins WHERE namespace=?1 AND key=?2 AND owner=?3",
                    params![namespace.as_str(), key, owner],
                )? == 0
                {
                    return Err(StoreError::new(
                        "NOT_OWNED",
                        "no matching durable pin owned by caller",
                    ));
                }
                json!({"version":1,"status":"OK"})
            }
            Command::Lease {
                version: v,
                namespace,
                key,
                owner,
                expires_at_ms,
            } => {
                version(v)?;
                text_id(&key, 8192, "key")?;
                require_entry(&tx, namespace.as_str(), &key)?;
                let lease = Lease {
                    owner,
                    expires_at_ms,
                };
                acquire_lease(&tx, namespace.as_str(), &key, &lease)?;
                json!({"version":1,"status":"OK","lease":lease_value(Some(&lease))})
            }
            Command::Release {
                version: v,
                namespace,
                key,
                owner,
            } => {
                version(v)?;
                validate_owned(&key, &owner)?;
                if tx.execute("UPDATE leases SET released=1 WHERE namespace=?1 AND key=?2 AND owner=?3 AND released=0", params![namespace.as_str(),key,owner])? == 0 {
                    return Err(StoreError::new("NOT_OWNED", "no matching active lease owned by caller"));
                }
                json!({"version":1,"status":"OK"})
            }
            Command::Evict {
                version: v,
                max_bytes,
            } => {
                version(v)?;
                integer(max_bytes)?;
                let mut response = evict(&tx, max_bytes)?;
                #[cfg(test)]
                crash_point("eviction_uncommitted");
                tx.commit()?;
                #[cfg(test)]
                crash_point("eviction_committed");
                let gc = self
                    .db
                    .transaction_with_behavior(TransactionBehavior::Immediate)?;
                response["removedBlobs"] = json!(collect_garbage(&gc, &root)?);
                gc.commit()?;
                return Ok(response);
            }
            Command::Budget {
                version: v,
                action,
                id,
                limits,
                amount,
            } => {
                version(v)?;
                text_id(&id, 256, "budget id")?;
                let result = budget(&tx, &action, &id, limits, amount)?;
                if action == "observe" && result["status"] == "ERROR" {
                    tx.commit()?;
                    let mut error = StoreError::new(
                        "BUDGET_EXCEEDED",
                        "observed spend persisted and exceeds limits",
                    );
                    error.details = Some(result["details"].clone());
                    return Err(error);
                }
                result
            }
        };
        tx.commit()?;
        #[cfg(test)]
        crash_point("manifest_committed");
        Ok(result)
    }
}
