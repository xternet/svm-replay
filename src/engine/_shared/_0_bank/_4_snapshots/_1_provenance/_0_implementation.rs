use super::*;

pub(in super::super) fn snapshot_context(snapshot: &Value) -> Check<()> {
    pubkey(field(snapshot, "genesisHash")?)?;
    pubkey(field(snapshot, "blockhash")?)?;
    require(
        slot(field(snapshot, "parentSlot")?)? < slot(field(snapshot, "slot")?)?,
        "parent ordering",
    )?;
    for key in ["blockEvidenceSha256", "activeFeatureSetHash"] {
        digest(field(snapshot, key)?)?;
    }
    for key in ["executorSourceId", "runtimeProfileId"] {
        identifier(field(snapshot, key)?, None)?;
    }
    Ok(())
}

pub(in super::super) fn bound_parts<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Check<(Value, &'a Value, &'a Value)> {
    let mut bound_fields = fields.to_vec();
    bound_fields.extend(["source", "proofHash"]);
    shape(value, &bound_fields)?;
    let mut snapshot = object(value)?.clone();
    snapshot.remove("source");
    snapshot.remove("proofHash");
    Ok((
        Value::Object(snapshot),
        field(value, "source")?,
        field(value, "proofHash")?,
    ))
}

pub(in super::super) fn source_and_proof(
    snapshot: &Value,
    source: &Value,
    proof: &Value,
    kinds: &[&str],
    domain: &str,
) -> Check<()> {
    shape(source, &["id", "kind", "evidenceSha256"])?;
    identifier(field(source, "id")?, Some(128))?;
    require(
        kinds.contains(&string(field(source, "kind")?)?),
        "explicit trusted producer kind required",
    )?;
    require(
        digest(field(source, "evidenceSha256")?)? == sha256(canonical_json(snapshot)),
        "source evidence/body mismatch",
    )?;
    let expected = sha256(format!(
        "{domain}\n{}\n",
        canonical_json(&json!({"snapshot":snapshot, "source":source}))
    ));
    require(digest(proof)? == expected, "bound proof hash mismatch")
}

pub(in super::super) fn clock_data(clock: &Value, target_slot: u64) -> Check<Vec<u8>> {
    require(
        field(clock, "pubkey")? == "SysvarC1ock11111111111111111111111111111111"
            && field(clock, "presence")? == "present"
            && field(clock, "sourceSlot")? == &json!(target_slot)
            && field(clock, "role")? == "sysvar"
            && field(clock, "executable")? == &json!(false)
            && field(clock, "owner")? == "Sysvar1111111111111111111111111111111111111",
        "exact target Clock required",
    )?;
    let bytes = bytes(field(clock, "dataBase64")?)?;
    require(bytes.len() == 40, "Clock must contain exactly 40 bytes")?;
    let recorded_slot = u64::from_le_bytes(
        bytes[..8]
            .try_into()
            .map_err(|error| format!("Clock slot: {error}"))?,
    );
    require(recorded_slot == target_slot, "Clock bytes/slot mismatch")?;
    Ok(bytes)
}

pub(in super::super) fn lineage(snapshot: &Value, bank: &Value) -> Check<()> {
    // JSON.stringify(lineage) in the prototype uses this insertion order, NOT stableJson.
    let encoded = format!("{{\"sourceHash\":{},\"targetSlot\":{},\"parentSlot\":{},\"blockhash\":{},\"previousBlockhash\":{}}}",
        field(snapshot, "blockEvidenceSha256")?, field(snapshot, "slot")?, field(snapshot, "parentSlot")?,
        field(snapshot, "blockhash")?, field(bank, "executionBlockhash")?);
    require(
        sha256(encoded) == string(field(bank, "blockLineageHash")?)?,
        "canonical block lineage context mismatch",
    )
}

pub(in super::super) fn runtime_matches(snapshot: &Value, binding: &Value) -> Check<()> {
    equal(
        field(snapshot, "slot")?,
        field(binding, "targetSlot")?,
        "runtime slot",
    )?;
    equal(
        field(snapshot, "executorSourceId")?,
        field(field(binding, "executor")?, "id")?,
        "executor source",
    )?;
    equal(
        field(snapshot, "runtimeProfileId")?,
        field(binding, "runtimeProfileId")?,
        "runtime profile",
    )?;
    equal(
        field(snapshot, "activeFeatureSetHash")?,
        field(binding, "activeFeatureSetHash")?,
        "active features",
    )
}
