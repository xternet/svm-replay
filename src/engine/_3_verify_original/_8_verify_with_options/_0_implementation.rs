use super::*;

/// General callers may explicitly allow a replacement with unchanged observable behavior.
pub fn verify_with_options(
    candidate: &Value,
    fixture: &Value,
    raw_block: &Value,
    output: &Value,
    metadata_policy: MetadataPolicy,
    require_replacement_change: bool,
) -> Result<VerificationReport> {
    let case_id = string(field(candidate, "id")?)?;
    let target_slot = integer(field(candidate, "slot")?)?;
    let target_index = usize_value(field(candidate, "transactionIndex")?)?;
    let signature = string(field(candidate, "targetSignature")?)?;
    let source_id = string(field(candidate, "executorSourceId")?)?;
    let block = field(raw_block, "result")?;
    let parent_slot = integer(field(block, "parentSlot")?)?;
    check(
        parent_slot < target_slot,
        "parent slot must precede target slot",
    )?;
    string(field(block, "blockhash")?)?;
    let archive = archived_transaction(raw_block, target_index)?;
    check(
        archive.signature == signature,
        "frozen target identity mismatch",
    )?;
    let block_hash = string(field(candidate, "blockSourceHash")?)?;
    equal(
        field(candidate, "rawEvidenceHash")?,
        &json!(hash(format!("{block_hash}\n{target_index}\n{signature}\n"))),
        "raw target evidence",
    )?;
    equal(
        field(fixture, "schema")?,
        &json!("svm-simulate-m6-fixture/v1"),
        "fixture schema",
    )?;
    equal(
        field(fixture, "caseId")?,
        &json!(case_id),
        "fixture case id",
    )?;
    let target = field(fixture, "target")?;
    for (name, expected) in [
        ("targetSlot", json!(target_slot)),
        ("parentSlot", json!(parent_slot)),
        ("index", json!(target_index)),
        ("signature", json!(signature)),
    ] {
        equal(field(target, name)?, &expected, &format!("fixture {name}"))?;
    }
    let binding = field(field(fixture, "runtime")?, "binding")?;
    equal(
        field(binding, "targetSlot")?,
        &json!(target_slot),
        "runtime target slot",
    )?;
    equal(
        field(binding, "runtimeProfileId")?,
        field(candidate, "runtimeProfileId")?,
        "runtime profile",
    )?;
    let executor = field(binding, "executor")?;
    equal(
        field(executor, "id")?,
        &json!(source_id),
        "runtime executor source",
    )?;
    let trace_required = match executor.get("m9Build") {
        None => false,
        Some(build) => {
            object(build)?;
            match build.get("capabilities") {
                None => false,
                Some(capabilities) => strings(capabilities)?
                    .iter()
                    .any(|value| value == "runtime-instruction-trace/v1"),
            }
        }
    };
    let mut evidence = strings(field(target, "evidenceAccounts")?)?;
    evidence.sort();
    check(
        evidence == archive.writable,
        "target evidence accounts mismatch",
    )?;
    let (end_accounts, fee_rewards) = state::end_transaction_accounts(fixture, block, target_slot)?;
    for (name, expected) in [
        ("schema", json!("svm-simulate-m6-executor-output/v1")),
        ("caseId", json!(case_id)),
        ("executorSourceId", json!(source_id)),
        ("runtimeBindingHash", field(binding, "bindingHash")?.clone()),
        ("targetIndex", json!(target_index)),
        ("targetSignature", json!(signature)),
    ] {
        equal(field(output, name)?, &expected, &format!("output {name}"))?;
    }
    let prefix = array(field(output, "prefix")?)?;
    let indices = array(field(target, "prefixIndices")?)?;
    let signatures = strings(field(target, "prefixSignatures")?)?;
    check(
        prefix.len() == indices.len() && signatures.len() == indices.len(),
        "prefix output length mismatch",
    )?;
    let mut divergences = Vec::new();
    let mut previous = None;
    for (position, (index, entry)) in indices.iter().zip(prefix).enumerate() {
        let index = usize_value(index)?;
        check(index < target_index, "prefix index not before target")?;
        if let Some(previous) = previous {
            check(index > previous, "unordered/duplicate prefix index")?;
        }
        previous = Some(index);
        require_trace(entry, trace_required)?;
        equal(field(entry, "index")?, &json!(index), "prefix index")?;
        equal(
            field(entry, "signature")?,
            &json!(signatures[position]),
            "prefix signature",
        )?;
        let archive = archived_transaction(raw_block, index)?;
        check(
            archive.signature == signatures[position],
            "prefix archived signature mismatch",
        )?;
        let label = format!("prefix {index}");
        verify_execution(
            &archive,
            &archive.writable,
            entry,
            &[],
            &label,
            metadata_policy,
            &mut divergences,
        )
        .map_err(|error| VerificationError(format!("{label}: {error}")))?;
    }
    let original = field(output, "original")?;
    require_trace(original, trace_required)?;
    let original_report = verify_execution(
        &archive,
        &evidence,
        original,
        &end_accounts,
        "original",
        metadata_policy,
        &mut divergences,
    )
    .map_err(|error| VerificationError(format!("original: {error}")))?;
    let mut expected_fees = Vec::new();
    for effect in array(field(target, "omittedFeeEffects")?)? {
        let mut projected = Map::new();
        for key in ["index", "signature", "feePayer", "feeLamports"] {
            projected.insert(key.into(), field(effect, key)?.clone());
        }
        expected_fees.push(Value::Object(projected));
    }
    equal(
        field(output, "omittedFeeEffects")?,
        &json!(expected_fees),
        "omitted fee application",
    )?;
    let requested_expected = object(fixture)?.contains_key("requestedAccountOverrides");
    let replacement_expected =
        !field(target, "replacementTransactionBase64")?.is_null() && !requested_expected;
    let replacement = nullable_object(field(output, "replacement")?)?;
    check(
        replacement.is_some() == replacement_expected,
        "replacement attempt mismatch",
    )?;
    let replacement_changed = if let Some(replacement) = replacement {
        require_trace(replacement, trace_required)?;
        assert_requested_execution_shape(replacement)?;
        execution_behavior(original)? != execution_behavior(replacement)?
    } else {
        false
    };
    check(
        !replacement_expected || replacement_changed || !require_replacement_change,
        "replacement produced no observable change",
    )?;
    let override_expected = nullable_object(field(fixture, "accountOverride")?)?;
    let account_override = nullable_object(field(output, "accountOverride")?)?;
    check(
        override_expected.is_some() == account_override.is_some(),
        "account override attempt mismatch",
    )?;
    let mut override_applied = false;
    if let (Some(expected), Some(actual)) = (override_expected, account_override) {
        require_trace(actual, trace_required)?;
        assert_requested_execution_shape(actual)?;
        let proof = field(expected, "proof")?;
        let pubkey = string(field(proof, "pubkey")?)?;
        for row in array(field(actual, "accountTransitions")?)? {
            if string(field(row, "pubkey")?)? == pubkey {
                override_applied =
                    field(field(row, "before")?, "dataHash")? == field(proof, "afterDataHash")?;
            }
        }
        check(override_applied, "account override was not applied")?;
    }
    let requested = match output.get("requestedOverrides") {
        None => None,
        Some(value) => nullable_object(value)?,
    };
    check(
        requested.is_some() == requested_expected,
        "requested override attempt mismatch",
    )?;
    let mut basis = json!({"caseId":case_id,"executorSourceId":source_id,"runtimeBindingHash":field(binding,"bindingHash")?,
        "prefixCount":prefix.len(),"original":original_report,
        "replacement":{"attempted":replacement_expected,"behaviorChanged":replacement_changed},
        "accountOverride":{"attempted":override_expected.is_some(),"appliedBeforeExecution":override_applied}});
    if let Some(requested) = requested {
        require_trace(requested, trace_required)?;
        assert_requested_execution_shape(requested)?;
        basis["requestedOverrides"] = requested_override_proof(fixture, requested)?;
    }
    if !fee_rewards.is_empty() {
        basis["endSlotFeeRewards"] = json!(fee_rewards);
    }
    let version = match metadata_policy {
        MetadataPolicy::Strict => "v1",
        MetadataPolicy::ArchivedComputeMeterWarning => {
            basis["metadataPolicy"] = json!(metadata_policy);
            basis["metadataDivergences"] = json!(divergences);
            "v2"
        }
    };
    let evidence_hash = hash(format!(
        "m6-verification/{version}\n{}\n",
        canonical_json(&basis)
    ));
    basis["schema"] = json!(format!("svm-simulate-m6-verification/{version}"));
    basis["status"] = json!("PASS");
    basis["evidenceHash"] = json!(evidence_hash);
    Ok(VerificationReport(basis))
}
