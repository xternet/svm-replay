use super::*;

fn request(after: u64) -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"getBlocksWithLimit","params":[after+1,1,{"commitment":"finalized"}]})
}
fn selected(raw: &[u8], after: u64, last: u64) -> Result<u64> {
    let value = parse(raw)?;
    let slots = envelope(&value, 1)?.as_array().ok_or_else(|| {
        SourceError::new("SOURCE_INTEGRITY", "next-block result must be an array")
    })?;
    require(
        !slots.is_empty(),
        "SOURCE_UNAVAILABLE",
        "no finalized produced block after the selected slot",
    )?;
    require(
        slots.len() == 1,
        "SOURCE_INTEGRITY",
        "next-block response exceeds its limit",
    )?;
    let next = slot(&slots[0], "SOURCE_INTEGRITY", "produced slot")?;
    require(
        next > after,
        "SOURCE_CONTEXT_MISMATCH",
        "produced slot does not follow boundary",
    )?;
    require(
        next <= last,
        "SOURCE_UNAVAILABLE",
        "next produced block is outside provider coverage",
    )?;
    Ok(next)
}

impl AlchemySource {
    /// First finalized produced block strictly after a slot, including skipped
    /// slots. Its parent link must independently confirm the selected boundary.
    pub fn discover_next_block(&self, after: u64) -> Result<Value> {
        self.discover_next_block_inner(after)
            .map_err(|error| attribute_source_failure(error, &self.identity))
    }
    fn discover_next_block_inner(&self, after: u64) -> Result<Value> {
        slot(&json!(after), "INVALID_QUERY", "after slot")?;
        let first = slot(
            &self.identity["coverage"]["firstSlot"],
            "SOURCE_INTEGRITY",
            "first slot",
        )?;
        let last = slot(
            &self.identity["coverage"]["lastSlot"],
            "SOURCE_INTEGRITY",
            "last slot",
        )?;
        require(
            after < last && after.checked_add(1).is_some_and(|n| n >= first),
            "SOURCE_UNAVAILABLE",
            "after-slot boundary is outside provider coverage",
        )?;
        let discovery=self.cached_discovery(json!({"kind":"next-block","afterSlot":after}), || {
            let mut state=self.state.lock().map_err(|_| SourceError::new("SOURCE_INTEGRITY","source state poisoned"))?;
            state.inspected=true;
            if state.genesis_raw.is_none() {
                let raw=self.post(&mut state,&json!({"jsonrpc":"2.0","id":1,"method":"getGenesisHash","params":[]}),false)?;
                checked_genesis(&raw,&self.identity["genesisHash"])?;
                state.genesis_raw=Some(raw);
            }
            let request=request(after);
            let raw=self.post(&mut state,&request,false)?;
            let next=selected(&raw,after,last)?;
            let genesis=state.genesis_raw.as_ref().ok_or_else(|| SourceError::new("SOURCE_INTEGRITY","next-block genesis missing"))?;
            Ok(json!({"schema":"svm-next-block-discovery/v1","afterSlot":after,"slot":next,"request":request,
                "responseBodyBase64":STANDARD.encode(&raw),"responseSha256":Digest::of(&raw),"genesisResponseBodyBase64":STANDARD.encode(genesis)}))
        }, |value| {
            object(value,&["schema","afterSlot","slot","request","responseBodyBase64","responseSha256","genesisResponseBodyBase64"],"SOURCE_INTEGRITY","next-block discovery")?;
            require(value["schema"]=="svm-next-block-discovery/v1" && value["afterSlot"]==after && value["request"]==request(after),"SOURCE_CONTEXT_MISMATCH","cached next-block request differs")?;
            let raw=canonical_base64(&value["responseBodyBase64"],"next-block response")?;
            let genesis=canonical_base64(&value["genesisResponseBodyBase64"],"next-block genesis")?;
            checked_genesis(&genesis,&self.identity["genesisHash"])?;
            require(value["responseSha256"]==json!(Digest::of(&raw)) && value["slot"]==selected(&raw,after,last)?,"SOURCE_INTEGRITY","cached next-block response differs")
        })?;
        let next = slot(
            &discovery["slot"],
            "SOURCE_INTEGRITY",
            "selected produced slot",
        )?;
        let record = self.discover_block(next)?;
        let raw = canonical_base64(&record["value"]["rawBase64"], "next block bytes")?;
        let block = parse(&raw)?;
        let parent = slot(
            &block["result"]["parentSlot"],
            "SOURCE_INTEGRITY",
            "next block parent slot",
        )?;
        require(
            parent <= after && after < next,
            "SOURCE_CONTEXT_MISMATCH",
            "next-block parent link skips a produced block",
        )?;
        Ok(record)
    }
}
