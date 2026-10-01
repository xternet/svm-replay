use super::*;

pub(super) fn bank_inputs(
    input: &HistoricalRequest,
    history: &mut History<'_>,
    a: &crate::_2_prepare_state::analysis::Analysis,
    source_hash: &Digest,
) -> Result<BTreeMap<String, Value>, Error> {
    let binding = &input.runtime_binding;
    let mut supplemental = BTreeMap::new();
    for name in &input.bank_inputs {
        let source=history.read(&json!({"kind":"bank-input","genesisHash":input.genesis_hash,
            "slot":a.slot,"parentSlot":a.parent,"blockhash":a.blockhash,"blockEvidenceSha256":source_hash,
            "executorSourceId":binding["executor"]["id"],"runtimeProfileId":binding["runtimeProfileId"],
            "activeFeatureSetHash":binding["activeFeatureSetHash"],"phase":"post-bank-initialization/pre-transaction","input":name}))?;
        if ["initializedStakeEvidence", "epochStakeEvidence"].contains(&name.as_str())
            && source["value"]["expectedGenesisHash"] != input.genesis_hash
        {
            return Err(Error::new(
                "SOURCE_CONTEXT_MISMATCH",
                "Bank evidence expected genesis differs",
            ));
        }
        supplemental.insert(name.clone(), source["value"].clone());
    }
    Ok(supplemental)
}
