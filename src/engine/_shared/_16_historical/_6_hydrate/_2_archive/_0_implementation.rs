use super::*;

impl SessionHydration {
    /// A finalized archive read through this session's own sources.
    pub(in super::super::super) fn archive(&mut self, query: &Value) -> Result<Value, Error> {
        self.sources
            .archive(query)
            .map_err(crate::shared::history::source_error)
    }
    /// An archived block by slot (pinned block discovery): the provider's getBlock result.
    pub(in super::super::super) fn archived_block(&mut self, slot: u64) -> Result<Value, Error> {
        let record = self
            .sources
            .discover_block(slot)
            .map_err(crate::shared::history::source_error)?;
        let raw = record["value"]["rawBase64"]
            .as_str()
            .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "discovered block bytes missing"))?;
        let block = svm_replay_protocol::parse_json(&crate::shared::history::data(raw)?)?;
        Ok(block["result"].clone())
    }
}
