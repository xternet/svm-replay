use super::*;

pub fn hydrate_override_sysvar_baselines_with_accounts<F>(
    overrides: &[RequestedAccountOverride],
    input_accounts: &[Value],
    context: &OverrideSysvarContext<'_>,
    mut fetch: F,
) -> Result<OverrideSysvarBaselines>
where
    F: FnMut(&str, u64) -> Result<Vec<Value>>,
{
    if overrides.iter().any(|o| o.pubkey == INSTRUCTIONS) {
        return Err(Error::new(
            "INVALID_REQUESTED_INPUT",
            "RUNTIME_DERIVED_SYSVAR_OVERRIDE",
        ));
    }
    let keys: Vec<_> = unique(overrides.iter().map(|o| o.pubkey.clone()))
        .into_iter()
        .filter(|key| {
            key != CLOCK
                && (key == SLOT_HASHES
                    || PARENT_SYSVARS.contains(&key.as_str())
                    || BANK_INITIALIZED_SYSVARS.contains(&key.as_str()))
        })
        .collect();
    let mut result = OverrideSysvarBaselines {
        accounts: input_accounts.to_vec(),
        bindings: vec![],
        generic_sysvars: vec![],
    };
    if keys.is_empty() {
        return Ok(result);
    }
    let commit = bank_source(context.executor_source_id).ok_or_else(|| {
        unsupported(
            "UNSUPPORTED_RUNTIME_PROFILE",
            "PREFLIGHT",
            format!(
                "unreviewed override sysvar phase:{}",
                context.executor_source_id
            ),
        )
    })?;
    features(context.active_execution_feature_ids)?;
    if !valid_boundary(context) {
        return Err(Error::new(
            "INVALID_REQUESTED_INPUT",
            "invalid override sysvar Bank boundary",
        ));
    }
    for key in keys {
        let slot = if key == SLOT_HASHES {
            context.target_slot
        } else {
            context.parent_slot
        };
        let matches: Vec<_> = result
            .accounts
            .iter()
            .enumerate()
            .filter(|(_, a)| a["pubkey"] == key)
            .map(|(i, _)| i)
            .collect();
        if matches.len() > 1 {
            return Err(Error::new(
                "INVALID_REQUESTED_INPUT",
                format!("duplicate override sysvar baseline:{key}"),
            ));
        }
        let mut image = matches.first().map(|index| result.accounts[*index].clone());
        if let Some(existing) = &image {
            let allowed = if key == SLOT_HASHES
                && existing["sourceSlot"].as_u64() == Some(context.parent_slot)
            {
                context.parent_slot
            } else {
                slot
            };
            assert_image(existing, allowed)?;
        }
        if image
            .as_ref()
            .is_none_or(|a| a["sourceSlot"].as_u64() != Some(slot))
        {
            let rows = fetch(&key, slot)?;
            if rows.len() != 1 || rows[0]["pubkey"] != key || rows[0]["presence"] != "present" {
                return Err(unsupported(
                    "UNSUPPORTED_BANK_INPUT",
                    "RECONSTRUCTION",
                    format!("UNSUPPORTED_EXACT_SYSVAR_UNAVAILABLE:{key}:override-baseline"),
                ));
            }
            let mut roles = Roles::default();
            roles.sysvars.insert(key.clone());
            let account = classify_account(rows[0].clone(), &key, slot, &roles)?;
            assert_image(&account, slot)?;
            if let Some(index) = matches.first() {
                result.accounts[*index] = account.clone();
            } else {
                result.accounts.push(account.clone());
            }
            image = Some(account);
        }
        let image = image.ok_or_else(|| {
            Error::new(
                "INVALID_REQUESTED_INPUT",
                format!("missing parsed override sysvar:{key}"),
            )
        })?;
        if key == SLOT_HASHES {
            result
                .generic_sysvars
                .push(bind_exact_generic_sysvar(&image, context.target_slot)?);
        }
        let phase = if key == SLOT_HASHES {
            "target-initialized"
        } else if PARENT_SYSVARS.contains(&key.as_str()) {
            "parent-end/pre-target"
        } else {
            "parent-input-for-bank-initialization"
        };
        result.bindings.push(json!({"policy":POLICY,"pubkey":key,"parentSlot":context.parent_slot,"targetSlot":context.target_slot,"sourceSlot":slot,"executorSourceId":context.executor_source_id,"bankSourceCommit":commit,"phase":phase,"accountSha256":Digest::of(canonical_json(&image).as_bytes()),"dataSha256":Digest::of(&account_data(&image)?)}));
    }
    Ok(result)
}

pub fn proven_parent_override_sysvars(
    accounts: &[Value],
    bindings: &[Value],
    context: &OverrideSysvarContext<'_>,
) -> Result<BTreeSet<String>> {
    let mut proven = BTreeSet::new();
    let parent: Vec<_> = bindings
        .iter()
        .filter(|b| b["phase"] == "parent-end/pre-target")
        .collect();
    if !parent.is_empty() {
        features(context.active_execution_feature_ids)?;
    }
    for b in parent {
        let reject = || {
            unsupported(
                "UNSUPPORTED_BANK_INPUT",
                "RECONSTRUCTION",
                "override sysvar baseline binding mismatch",
            )
        };
        let key = b["pubkey"].as_str().ok_or_else(reject)?;
        let matching: Vec<_> = accounts.iter().filter(|a| a["pubkey"] == key).collect();
        if matching.len() != 1
            || proven.contains(key)
            || !PARENT_SYSVARS.contains(&key)
            || b["policy"] != POLICY
            || bank_source(context.executor_source_id).is_none()
            || b["executorSourceId"] != context.executor_source_id
            || b["bankSourceCommit"].as_str() != bank_source(context.executor_source_id)
            || !valid_boundary(context)
            || b["parentSlot"].as_u64() != Some(context.parent_slot)
            || b["targetSlot"].as_u64() != Some(context.target_slot)
            || b["sourceSlot"].as_u64() != Some(context.parent_slot)
        {
            return Err(reject());
        }
        let a = matching[0];
        if b["accountSha256"] != json!(Digest::of(canonical_json(a).as_bytes()))
            || b["dataSha256"] != json!(Digest::of(&account_data(a).map_err(|_| reject())?))
        {
            return Err(reject());
        }
        assert_image(a, context.parent_slot)?;
        proven.insert(key.to_owned());
    }
    Ok(proven)
}
