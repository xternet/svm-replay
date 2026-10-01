use crate::shared::history::{classify_account, History, Roles};
use serde_json::{json, Value};
use svm_replay_protocol::{Digest, Error};
mod _0_slot_hashes;
#[cfg(test)]
mod tests;
pub(crate) fn hydrate_exact(
    history: &mut History<'_>,
    key: &str,
    slot: u64,
    fixture: &Value,
) -> Result<(Value, Vec<Digest>), Error> {
    let observation = history.read(&json!({"kind":"account","genesisHash":history.genesis,
        "slot":slot,"pubkey":key,"phase":"end-slot"}))?;
    let mut roles = Roles::default();
    roles.sysvars.insert(key.into());
    let mut hashes = serde_json::from_value::<Vec<Digest>>(observation["evidenceHashes"].clone())
        .map_err(|e| Error::new("SOURCE_INTEGRITY", e.to_string()))?;
    if hashes.is_empty() {
        return Err(Error::new(
            "SOURCE_INTEGRITY",
            "hydrated input lacks provenance",
        ));
    }
    if key == crate::shared::bank::slot_hashes::SLOT_HASHES
        && observation["value"]["presence"] == "absent"
    {
        let (proof, evidence) = _0_slot_hashes::recover(history, fixture)?;
        hashes.extend(evidence);
        return Ok((proof, hashes));
    }
    let account = classify_account(observation["value"].clone(), key, slot, &roles)?;
    Ok((account, hashes))
}
