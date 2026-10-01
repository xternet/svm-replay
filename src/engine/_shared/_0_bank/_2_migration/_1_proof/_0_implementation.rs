use super::*;

pub(in super::super) fn prove(
    input: &Value,
    migration: &Migration,
    activation: u64,
    parents: &mut BTreeMap<String, Value>,
    initialized: &mut BTreeMap<String, Value>,
) -> Result<Value> {
    let evidence = field(input, "evidence")?;
    let runtime = field(input, "runtime")?;
    let features = array(field(runtime, "features")?)?;
    check(
        field(field(runtime, "executor")?, "id")? == migration.executor
            && !has_feature(features, ALPENGLOW),
        "unreviewed migration operation/Bank era",
    )?;
    let kind = field(evidence, "evidenceKind")?;
    check(
        kind == "provider-attested" || kind == "controlled-bank",
        "evidence kind",
    )?;
    let slot = integer(field(input, "slot")?)?;
    let parent = integer(field(input, "parentSlot")?)?;
    let source = field(evidence, "block")?;
    let envelope = receipt(source, "getBlock")?;
    let block = field(&envelope, "result")?;
    let params = array(field(field(source, "request")?, "params")?)?;
    let options = &params[1];
    check(
        params[0] == slot
            && field(options, "encoding")? == "json"
            && field(options, "transactionDetails")? == "full"
            && field(options, "rewards")? == &json!(true)
            && field(options, "commitment")? == "finalized"
            && matches!(
                field(options, "maxSupportedTransactionVersion")?.as_u64(),
                Some(0 | 1)
            )
            && field(source, "responseSha256")? == field(input, "blockSourceHash")?
            && canonical_json(&envelope) == canonical_json(field(input, "envelope")?)
            && uint(field(block, "parentSlot")?)? == parent,
        "complete canonical block binding",
    )?;
    let affected = migration.keys();
    let mut signatures = BTreeSet::new();
    for (index, tx) in array(field(block, "transactions")?)?.iter().enumerate() {
        let version = field(tx, "version")?;
        check(
            version == "legacy" || version == &json!(0),
            "transaction version",
        )?;
        if version == &json!(0) {
            let lookups = array(field(
                field(field(tx, "transaction")?, "message")?,
                "addressTableLookups",
            )?)?;
            let loaded = field(field(tx, "meta")?, "loadedAddresses")?;
            for (indices, addresses) in [
                ("writableIndexes", "writable"),
                ("readonlyIndexes", "readonly"),
            ] {
                let expected = lookups
                    .iter()
                    .try_fold(0usize, |sum, row| -> Result<usize> {
                        sum.checked_add(array(field(row, indices)?)?.len())
                            .ok_or_else(|| fail("lookup count overflow"))
                    })?;
                check(
                    expected == array(field(loaded, addresses)?)?.len(),
                    "incomplete lookup resolution",
                )?;
            }
        }
        let semantic = summarize_semantic_transaction(tx, index as u64)?;
        check(
            signatures.insert(semantic.signature),
            "duplicate block transaction",
        )?;
        check(
            !semantic
                .writable_accounts
                .iter()
                .any(|id| affected.contains(&id.as_str())),
            "transaction writer",
        )?;
    }
    for reward in array(field(block, "rewards")?)? {
        object(reward)?;
        check(
            !reward
                .get("pubkey")
                .and_then(Value::as_str)
                .is_some_and(|id| affected.contains(&id)),
            "reward/fee writer",
        )?;
    }
    let entries = array(field(evidence, "accounts")?)?;
    let mut seen = BTreeSet::new();
    check(entries.len() == 3, "exact migration account set")?;
    for entry in entries {
        let id = string(field(entry, "pubkey")?)?;
        check(
            affected.contains(&id) && seen.insert(id),
            "exact migration account set",
        )?;
        parents.insert(
            id.into(),
            account(field(entry, "parentAccount")?, id, parent, migration)?,
        );
        initialized.insert(
            id.into(),
            account(field(entry, "targetAccount")?, id, slot, migration)?,
        );
    }
    let old_program = get(parents, migration.program)?;
    let old_data = get(parents, migration.data)?;
    let source = get(parents, migration.buffer)?;
    let program = get(initialized, migration.program)?;
    let pd = get(initialized, migration.data)?;
    let buffer = get(initialized, migration.buffer)?;
    let source_bytes = data(source)?;
    let pd_bytes = data(pd)?;
    let program_bytes = data(program)?;
    check(
        field(source, "owner")? == LOADER
            && field(source, "executable")? == &json!(false)
            && source_bytes.len() > 37
            && source_bytes[..4] == 1u32.to_le_bytes()
            && [0, 1].contains(&source_bytes[4]),
        "source Buffer layout",
    )?;
    check(
        field(program, "owner")? == LOADER
            && field(program, "executable")? == &json!(true)
            && program_bytes.len() == 36
            && history::program_data_address(program)?.as_deref() == Some(migration.data),
        "migrated Program pointer",
    )?;
    let mut expected = vec![0u8; 45];
    expected[..4].copy_from_slice(&3u32.to_le_bytes());
    expected[4..12].copy_from_slice(&slot.to_le_bytes());
    let token = migration.feature == MIGRATIONS[0].feature;
    if token {
        check(
            field(old_program, "owner")? == "BPFLoader2111111111111111111111111111111111"
                && field(old_program, "executable")? == &json!(true),
            "not a loader-v2 migration source",
        )?;
        check(
            field(old_data, "presence")? == "absent"
                || (old_data.get("owner") == Some(&json!(SYSTEM)) && has_feature(features, RELAX)),
            "unproven prefunded ProgramData allowance",
        )?;
    } else {
        let mut unchanged = program.clone();
        unchanged["sourceSlot"] = field(old_program, "sourceSlot")?.clone();
        check(
            canonical_json(&unchanged) == canonical_json(old_program),
            "core Program changed",
        )?;
        let old = data(old_data)?;
        check(
            field(old_data, "owner")? == LOADER
                && old.len() >= 45
                && old[..4] == 3u32.to_le_bytes()
                && [0, 1].contains(&old[12]),
            "old core ProgramData layout",
        )?;
        if old[12] == 1 {
            check(
                source_bytes[4] == 1 && source_bytes[5..37] == old[13..45],
                "core Buffer authority mismatch",
            )?;
            expected[12..45].copy_from_slice(&old[12..45]);
        }
    }
    expected.extend_from_slice(&source_bytes[37..]);
    check(
        field(pd, "owner")? == LOADER
            && field(pd, "executable")? == &json!(false)
            && pd_bytes == expected
            && field(buffer, "presence")? == "absent",
        "successful migration images/slot/authority/Buffer clearing",
    )?;
    let rent = account(field(evidence, "rent")?, RENT, slot, migration)?;
    let rent_bytes = data(&rent)?;
    check(
        field(&rent, "owner")? == sysvars::SYSVAR_OWNER && rent_bytes.len() == 17,
        "target Rent layout",
    )?;
    let rate = u64::from_le_bytes(
        rent_bytes[..8]
            .try_into()
            .map_err(|e| fail(format!("Rent rate:{e}")))?,
    );
    let threshold = f64::from_le_bytes(
        rent_bytes[8..16]
            .try_into()
            .map_err(|e| fail(format!("Rent threshold:{e}")))?,
    );
    check(
        threshold.is_finite()
            && threshold.fract() == 0.0
            && threshold > 0.0
            && threshold <= 9_007_199_254_740_991.0,
        "unreviewed Rent representation",
    )?;
    let minimum = |length: usize| -> Result<String> {
        let value = (length as u128 + 128)
            .checked_mul(u128::from(rate))
            .and_then(|v| v.checked_mul(threshold as u128))
            .ok_or_else(|| fail("migration rent funding exceeds representable u64"))?;
        // A value beyond u64 cannot equal the strict target account's lamports.
        check(
            value <= u128::from(u64::MAX),
            "migration rent funding exceeds u64",
        )?;
        Ok(value.to_string())
    };
    check(
        (!token || field(program, "lamports")? == &json!(minimum(program_bytes.len())?))
            && field(pd, "lamports")? == &json!(minimum(pd_bytes.len())?),
        "migration target rent funding",
    )?;
    let hashes = |values: &BTreeMap<String, Value>| -> Result<Vec<Value>> {
        let mut keys = affected;
        keys.sort();
        keys.iter()
            .map(|id| Ok(json!({"pubkey":id,"sha256":hash(canonical_json(get(values,id)?))})))
            .collect()
    };
    let mut proof = json!({"schema":"svm-program-migration-initialization/v1","evidenceKind":kind,"featureId":migration.feature,"activationSlot":activation,
        "program":migration.program,"programData":migration.data,"buffer":migration.buffer,"parentSlot":parent,"targetSlot":slot,
        "executorSourceId":field(field(runtime,"executor")?,"id")?,"blockSourceHash":field(input,"blockSourceHash")?,
        "parentAccountHashes":hashes(parents)?,"targetAccountHashes":hashes(initialized)?,"rentAccountHash":hash(canonical_json(&rent))});
    proof["proofHash"] = json!(hash(format!(
        "program-migration-initialization/v1\n{}\n",
        canonical_json(&proof)
    )));
    Ok(proof)
}
