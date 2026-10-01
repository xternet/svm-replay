use super::*;

pub(in super::super) fn verify_execution(
    archive: &ArchivedTransaction<'_>,
    evidence: &[String],
    execution: &Value,
    end_accounts: &[Value],
    label: &str,
    policy: MetadataPolicy,
    divergences: &mut Vec<MetadataDivergence>,
) -> Result<Value> {
    let meta = archive.meta;
    let unexecuted = crate::shared::instructions::is_unexecuted_load_failure(meta);
    let empty = json!([]);
    let instruction_trace = match execution.get("instructionTrace") {
        None => json!({"status":"NOT_CAPTURED"}),
        Some(trace) => verify_runtime_instruction_trace(
            trace,
            if unexecuted {
                &empty
            } else {
                field(meta, "innerInstructions")?
            },
        )?,
    };
    let error = field(meta, "err")?;
    equal(
        field(execution, "status")?,
        &json!(if error.is_null() { "ok" } else { "err" }),
        "status",
    )
    .map_err(|error| {
        VerificationError(format!(
            "{error}; runtime error: {}",
            execution
                .get("normalizedError")
                .map(Value::to_string)
                .unwrap_or_else(|| "missing".into())
        ))
    })?;
    if error.is_null() {
        check(
            field(execution, "normalizedError")?.is_null(),
            "normalized error mismatch",
        )?;
    } else {
        let encoded = string(field(execution, "normalizedError")?)?;
        // The worker historically emits either a JSON-encoded error or a plain enum name.
        let parsed = match serde_json::from_str::<Value>(encoded) {
            Ok(parsed) => parsed,
            Err(_) => Value::String(encoded.into()),
        };
        check(
            canonical_json(&parsed) == canonical_json(error),
            "normalized error mismatch",
        )?;
    }
    let canonical_logs = strings(if unexecuted {
        &empty
    } else {
        field(meta, "logMessages")?
    })?;
    let logs = strings(field(execution, "logs")?)?;
    metadata::complete_logs(&canonical_logs)?;
    metadata::complete_logs(&logs)?;
    let compute_units = integer(field(execution, "computeUnits")?)?;
    let canonical_compute_units = integer(field(meta, "computeUnitsConsumed")?)?;
    match policy {
        MetadataPolicy::Strict => {
            check(logs == canonical_logs, "logs mismatch")?;
            check(
                compute_units == canonical_compute_units,
                "compute units mismatch",
            )?;
        }
        MetadataPolicy::ArchivedComputeMeterWarning => {
            if let Some(divergence) = compare_archived_compute_metadata(
                label,
                &canonical_logs,
                &logs,
                canonical_compute_units,
                compute_units,
            )? {
                divergences.push(divergence);
            }
        }
    }
    let fee = unsigned(field(execution, "fee")?)?;
    check(fee == unsigned(field(meta, "fee")?)?, "fee mismatch")?;
    equal(
        &return_data(execution.get("returnData"), false)?,
        &return_data(meta.get("returnData"), true)?,
        "return data",
    )?;
    let pre_balances: Vec<_> = array(field(meta, "preBalances")?)?
        .iter()
        .map(unsigned)
        .collect::<Result<_>>()?;
    let post_balances: Vec<_> = array(field(meta, "postBalances")?)?
        .iter()
        .map(unsigned)
        .collect::<Result<_>>()?;
    check(
        pre_balances.len() == archive.keys.len() && post_balances.len() == archive.keys.len(),
        "canonical balance/account length mismatch",
    )?;
    let pre_tokens = token_amounts(field(meta, "preTokenBalances")?, archive.keys.len())?;
    let post_tokens = token_amounts(field(meta, "postTokenBalances")?, archive.keys.len())?;
    // Agave BalanceCollector omits token metadata when neither token program
    // appears in the message, even if a declared account is token-owned.
    let token_metadata_recorded = !pre_tokens.is_empty()
        || !post_tokens.is_empty()
        || archive.keys.iter().any(|key| {
            matches!(
                key.as_str(),
                "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
                    | "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
            )
        });
    let transitions = array(field(execution, "accountTransitions")?)?;
    check(
        transitions.len() == evidence.len(),
        "transition count mismatch",
    )?;
    let mut by_pubkey = BTreeMap::new();
    for transition in transitions {
        let pubkey = string(field(transition, "pubkey")?)?;
        check(
            by_pubkey.insert(pubkey, transition).is_none(),
            "duplicate transition",
        )?;
    }
    let mut tokens = 0usize;
    for pubkey in evidence {
        let row = by_pubkey
            .get(pubkey.as_str())
            .ok_or_else(|| VerificationError(format!("transition missing {pubkey}")))?;
        let index = archive
            .keys
            .iter()
            .position(|key| key == pubkey)
            .ok_or_else(|| VerificationError(format!("canonical account missing {pubkey}")))?;
        let before = field(row, "before")?;
        let after = field(row, "after")?;
        check(
            state_lamports(before)? == pre_balances[index],
            &format!("{pubkey} pre lamports mismatch"),
        )?;
        check(
            state_lamports(after)? == post_balances[index],
            &format!("{pubkey} post lamports mismatch"),
        )?;
        let pre_token = pre_tokens.get(&index).copied();
        let post_token = post_tokens.get(&index).copied();
        if pre_token.is_some() || post_token.is_some() {
            tokens += 1;
        }
        let before_token = nullable_unsigned(field(before, "tokenAmount")?)?;
        let after_token = nullable_unsigned(field(after, "tokenAmount")?)?;
        if token_metadata_recorded {
            check(before_token == pre_token, "pre token mismatch")?;
            check(after_token == post_token, "post token mismatch")?;
        }
    }
    let mut expected_end = BTreeMap::new();
    for expected in end_accounts {
        check(
            expected_end
                .insert(string(field(expected, "pubkey")?)?, expected)
                .is_none(),
            "duplicate end account",
        )?;
    }
    let actual_end = array(field(execution, "endAccountStates")?)?;
    check(
        actual_end.len() == expected_end.len(),
        "end-state count mismatch",
    )?;
    for named in actual_end {
        let pubkey = string(field(named, "pubkey")?)?;
        let expected = expected_end
            .remove(pubkey)
            .ok_or_else(|| VerificationError(format!("unexpected/duplicate end state {pubkey}")))?;
        archived_state(expected, field(named, "state")?)?;
    }
    check(expected_end.is_empty(), "end-state evidence missing")?;
    Ok(
        json!({"status":true,"normalizedError":true,"logs":logs.len(),"computeUnits":compute_units,"fee":fee.to_string(),
        "returnData":true,"instructionTrace":instruction_trace,"immediateAccounts":transitions.len(),
        "immediateLamports":transitions.len(),"immediateTokenAmounts":tokens,
        "tokenBalanceMetadata":if token_metadata_recorded {"CHECKED"} else {"NOT_RECORDED_NO_TOKEN_PROGRAM"},
        "endAccountStates":actual_end.len()}),
    )
}
