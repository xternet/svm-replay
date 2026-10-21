use super::*;

fn no_archive() -> Error {
    Error::new(
        "HISTORICAL_DATA_UNAVAILABLE",
        "this session keeps no archive inputs",
    )
}

impl VerifiedSession {
    /// The boundary block as archived, its slot, and the fork's position in it: the index of
    /// the block's first transaction that did not happen on this fork.
    pub fn boundary_block(&self) -> Result<(std::sync::Arc<Value>, u64, u64), Error> {
        Ok((
            self.boundary.clone(),
            integer(&self.pins["slot"])?,
            integer(&self.pins["transactionIndex"])?,
        ))
    }
    /// A finalized archive read through this session's sources.
    pub fn archive(&mut self, query: &Value) -> Result<Value, Error> {
        self.hydration
            .as_mut()
            .ok_or_else(no_archive)?
            .archive(query)
    }
    /// An archived block by slot through this session's sources.
    pub fn archived_block(&mut self, slot: u64) -> Result<Value, Error> {
        self.hydration
            .as_mut()
            .ok_or_else(no_archive)?
            .archived_block(slot)
    }
}
