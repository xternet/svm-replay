use super::*;

/// Re-call with an expanded requestedPubkeys frontier; no unseen source reads are performed.
pub fn program_migration_context(input: &Value) -> Result<Value> {
    let build = || -> Result<Value> {
        let slot = integer(field(input, "slot")?)?;
        let parent = integer(field(input, "parentSlot")?)?;
        check(parent < slot, "migration parent order")?;
        let requested = array(field(input, "requestedPubkeys")?)?;
        for id in requested {
            pubkey(id)?;
        }
        let features = array(field(field(input, "runtime")?, "features")?)?;
        let mut initialized = BTreeMap::new();
        let mut parents = BTreeMap::new();
        let mut proofs = Vec::new();
        for migration in &MIGRATIONS {
            if let Some(feature) = features
                .iter()
                .find(|feature| feature.get("id") == Some(&json!(migration.feature)))
            {
                let activation = integer(field(feature, "activationSlot")?)?;
                if activation > parent
                    && activation <= slot
                    && migration
                        .keys()
                        .iter()
                        .any(|id| requested.iter().any(|v| v == id))
                {
                    proofs.push(prove(
                        input,
                        migration,
                        activation,
                        &mut parents,
                        &mut initialized,
                    )?);
                }
            }
        }
        Ok(
            json!({"initialized":initialized.into_values().collect::<Vec<_>>(),"parents":parents.into_values().collect::<Vec<_>>(),
            "proofs":proofs,"parentSlot":parent,"evidenceSupplied":input.get("evidence").is_some()}),
        )
    };
    build().map_err(error)
}

pub fn resolve_program_migration_overlays(
    context: &Value,
    pubkeys: &[String],
    parent_links: &Value,
) -> Result<Value> {
    let resolve = || -> Result<Value> {
        let initialized = array(field(context, "initialized")?)?;
        let links = object(parent_links)?;
        let mut result = Map::new();
        for id in pubkeys {
            pubkey(&json!(id))?;
            check(!result.contains_key(id), "duplicate resolution key")?;
            let link = if let Some(account) = initialized
                .iter()
                .find(|v| v.get("pubkey") == Some(&json!(id)))
            {
                json!(history::program_data_address(account)?)
            } else {
                let value = links
                    .get(id)
                    .ok_or_else(|| fail(format!("parent ProgramData link unavailable:{id}")))?;
                if !value.is_null() {
                    pubkey(value)?;
                }
                value.clone()
            };
            result.insert(id.clone(), link);
        }
        Ok(Value::Object(result))
    };
    resolve().map_err(error)
}

pub fn validate_program_migration_parents(context: &Value, records: &[Value]) -> Result<()> {
    let validate = || -> Result<()> {
        let proofs = array(field(context, "proofs")?)?;
        check(
            field(context, "evidenceSupplied")? == &json!(false) || !proofs.is_empty(),
            "unrelated migration evidence",
        )?;
        let slot = integer(field(context, "parentSlot")?)?;
        for expected in array(field(context, "parents")?)? {
            let id = string(field(expected, "pubkey")?)?;
            let mut matches = records
                .iter()
                .filter(|v| v.get("pubkey") == Some(&json!(id)));
            let observed = matches
                .next()
                .ok_or_else(|| fail(format!("parent input missing:{id}")))?;
            check(matches.next().is_none(), "duplicate parent input")?;
            let migration = MIGRATIONS
                .iter()
                .find(|m| {
                    m.keys().contains(&id)
                        && proofs
                            .iter()
                            .any(|p| p.get("featureId") == Some(&json!(m.feature)))
                })
                .ok_or_else(|| fail("parent migration identity"))?;
            check(
                canonical_json(&strict(observed, id, slot, migration)?) == canonical_json(expected),
                "prepared parent differs",
            )?;
        }
        Ok(())
    };
    validate().map_err(error)
}
