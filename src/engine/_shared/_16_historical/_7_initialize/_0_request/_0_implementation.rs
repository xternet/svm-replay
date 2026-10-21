use super::*;

/// Canonical slot initialization has no archived target or original transaction.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitializationRequest {
    pub schema: String,
    pub request_id: String,
    pub family: String,
    pub genesis_hash: String,
    pub after_slot: u64,
    pub slot: u64,
    pub raw_block_base64: String,
    pub block_sha256: Digest,
    pub runtime_binding: Value,
    pub extra_accounts: Vec<String>,
    pub limits: Limits,
}

pub(in super::super) fn position(block: &Value, after: u64, slot: u64) -> Result<u64, Error> {
    let parent = block["result"]["parentSlot"]
        .as_u64()
        .ok_or_else(|| invalid("initialized block parent missing"))?;
    if slot > 9_007_199_254_740_991
        || !(parent <= after && after < slot)
        || !block["result"]["transactions"].is_array()
    {
        return Err(invalid(
            "initialized block is not the produced successor of the requested slot",
        ));
    }
    Ok(parent)
}

impl InitializationRequest {
    pub fn validate(&self) -> Result<Value, Error> {
        self.limits.validate()?;
        history::address(&self.genesis_hash, 32)?;
        if self.schema != "svm-historical-initialization-request/v1"
            || self.request_id.is_empty()
            || self.request_id.len() > 200
            || !self
                .request_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            || !["v2-2", "v2-3", "v3-0", "v3-1", "v4-0", "v4-1", "v4-2"]
                .contains(&self.family.as_str())
        {
            return Err(invalid("invalid initialization request identity"));
        }
        let raw = history::data(&self.raw_block_base64)?;
        if Digest::of(&raw) != self.block_sha256 {
            return Err(invalid(
                "initialized block bytes differ from their source pin",
            ));
        }
        let block = svm_replay_protocol::parse_json(&raw)?;
        position(&block, self.after_slot, self.slot)?;
        svm_replay_protocol::runtime::validate_runtime_binding(&self.runtime_binding)?;
        if self.runtime_binding["targetSlot"] != self.slot {
            return Err(invalid("initialized runtime slot differs"));
        }
        let mut seen = BTreeSet::new();
        for key in &self.extra_accounts {
            history::address(key, 32)?;
            if !seen.insert(key) {
                return Err(invalid("duplicate initialization account"));
            }
        }
        Ok(block)
    }
}
