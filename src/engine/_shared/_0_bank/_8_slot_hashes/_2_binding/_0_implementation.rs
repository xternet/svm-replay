use super::*;

/// Validate a data-only binding. Authenticity comes from the pinned reconstruction
/// observations (or explicit caller-supplied inputs), not this structural check.
pub fn bind_data(proof: &Value, target: u64) -> Result<Value> {
    let fields = [
        "schema",
        "scope",
        "pubkey",
        "sourceSlot",
        "targetBlockhash",
        "executorSourceId",
        "dataBase64",
        "dataSha256",
        "dataLen",
        "evidence",
    ];
    let value = object(proof)?;
    check(
        value.len() == fields.len() && fields.iter().all(|k| value.contains_key(*k)),
        "SlotHashes data-only proof fields differ",
    )?;
    check(
        proof["schema"] == "vote-derived-slot-hashes/v1"
            && proof["scope"] == "sysvar-data-only"
            && proof["pubkey"] == SLOT_HASHES
            && proof["sourceSlot"] == target,
        "SlotHashes data-only identity differs",
    )?;
    reviewed(string(field(proof, "executorSourceId")?)?)?;
    pubkey(field(proof, "targetBlockhash")?)?;
    let data = bytes(field(proof, "dataBase64")?)?;
    check(
        data.len() == 8 + ENTRIES * 40
            && proof["dataLen"] == data.len()
            && proof["dataSha256"] == hash(&data),
        "SlotHashes data-only length/digest differs",
    )?;
    check(
        data[..8] == (ENTRIES as u64).to_le_bytes(),
        "SlotHashes entry count differs",
    )?;
    let mut previous = target;
    for entry in data[8..].chunks_exact(40) {
        let slot = u64::from_le_bytes(
            entry[..8]
                .try_into()
                .map_err(|_| fail("SlotHashes slot bytes"))?,
        );
        check(
            slot < previous,
            "SlotHashes ancestors not strictly descending",
        )?;
        previous = slot;
    }
    let evidence = array(field(proof, "evidence")?)?;
    check(
        (ENTRIES + 1..=ENTRIES * 2 + 1).contains(&evidence.len()),
        "SlotHashes proof chain size",
    )?;
    let mut previous = None;
    let mut target_seen = false;
    let mut ancestors = 0;
    for row in evidence {
        let slot = integer(field(row, "slot")?)?;
        check(previous.is_none_or(|p| slot < p), "SlotHashes proof order")?;
        previous = Some(slot);
        target_seen |= slot == target;
        ancestors += usize::from(slot < target);
        digest(string(field(row, "blockSha256")?)?)?;
        reviewed(string(field(row, "executorSourceId")?)?)?;
    }
    check(
        target_seen && ancestors == ENTRIES,
        "SlotHashes proof target/ancestors differ",
    )?;
    Ok(
        json!({"pubkey":SLOT_HASHES,"sourceSlot":target,"dataSha256":hash(&data),"dataLen":data.len()}),
    )
}

pub fn bind_fixture(fixture: &Value) -> Result<Value> {
    let target = integer(&fixture["target"]["targetSlot"])?;
    let runtime = field(fixture, "runtime")?;
    let proof = field(runtime, "slotHashesData")?;
    let binding = bind_data(proof, target)?;
    check(
        runtime["bankContext"]["targetSlot"] == target
            && proof["executorSourceId"] == runtime["binding"]["executor"]["id"],
        "SlotHashes proof runtime/block differs",
    )?;
    crate::shared::bank::context::assert_target_blockhash(
        &runtime["bankContext"],
        string(field(proof, "targetBlockhash")?)?,
    )?;
    for name in ["accounts", "endAccounts"] {
        if let Some(accounts) = fixture.get(name) {
            check(
                !array(accounts)?.iter().any(|a| a["pubkey"] == SLOT_HASHES),
                "SlotHashes data-only proof conflicts with full account",
            )?;
        }
    }
    Ok(binding)
}
