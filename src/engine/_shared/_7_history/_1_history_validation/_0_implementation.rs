use super::*;

impl<'a> History<'a> {
    pub fn new(
        sources: &'a mut CompositeSource,
        genesis: String,
        target_slot: u64,
        budget: &'a ExecutionBudget,
        max_reads: usize,
    ) -> Result<Self, Error> {
        address(&genesis, 32)?;
        if target_slot > 9_007_199_254_740_991 || max_reads == 0 {
            return Err(Error::new(
                "INVALID_REQUEST",
                "invalid history slot/read limit",
            ));
        }
        Ok(Self {
            sources,
            genesis,
            target_slot,
            budget,
            max_reads,
            reads: 0,
        })
    }
    pub fn read(&mut self, query: &Value) -> Result<Value, Error> {
        self.acquire(query, |sources| sources.require(query))
    }
    /// Finalized block discovery for evidence reconstruction, including later
    /// witnesses. This does not relax the prohibition on future account state.
    pub fn discover_block(&mut self, slot: u64) -> Result<Value, Error> {
        let query = json!({"kind":"block-discovery","slot":slot,"genesisHash":self.genesis});
        self.acquire(&query, |sources| sources.discover_block(slot))
    }
    fn acquire(
        &mut self,
        query: &Value,
        operation: impl FnOnce(&mut CompositeSource) -> Result<Value, SourceError>,
    ) -> Result<Value, Error> {
        self.budget
            .check()
            .map_err(super::super::super::runtime::protocol_error)?;
        if self.reads >= self.max_reads {
            return Err(Error::new(
                "UNSUPPORTED_RESOURCE_LIMIT",
                "historical read limit exhausted",
            ));
        }
        self.reads += 1;
        let result = operation(self.sources).map_err(source_error);
        // Synchronous providers may finish after cancellation/deadline. Retain
        // their error as context, but never admit their late result to execution.
        if let Err(error) = self.budget.check() {
            return Err(super::super::super::runtime::protocol_error(error)
                .with_details(json!({"query":query,"sourceError":result.err()})));
        }
        result
    }
    pub fn account(&mut self, key: &str, slot: u64, roles: &Roles) -> Result<Value, Error> {
        if slot > self.target_slot {
            return Err(Error::new(
                "SOURCE_CONTEXT_MISMATCH",
                "account query after target",
            ));
        }
        let value=self.read(&json!({"kind":"account","genesisHash":self.genesis,"slot":slot,"pubkey":key,"phase":"end-slot"}))?;
        classify_account(value["value"].clone(), key, slot, roles)
    }
    pub fn accounts(
        &mut self,
        keys: &[String],
        slot: u64,
        roles: &Roles,
    ) -> Result<Vec<Value>, Error> {
        keys.iter()
            .map(|key| self.account(key, slot, roles))
            .collect()
    }
    pub fn program_links(
        &mut self,
        keys: &[String],
        parent: u64,
    ) -> Result<BTreeMap<String, Option<String>>, Error> {
        let mut links = BTreeMap::new();
        for key in keys {
            let link = if [CLOCK, INSTRUCTIONS, RECENT_BLOCKHASHES].contains(&key.as_str()) {
                None
            } else {
                program_data_address(&self.account(key, parent, &Roles::default())?)?
            };
            if links.insert(key.clone(), link).is_some() {
                return Err(invalid("duplicate dependency key"));
            }
        }
        Ok(links)
    }
    pub fn block(&mut self, evidence: &Digest) -> Result<(Vec<u8>, Value), Error> {
        let source=self.read(&json!({"kind":"block","genesisHash":self.genesis,"slot":self.target_slot,"blockEvidenceSha256":evidence}))?;
        let bytes = data(
            source["value"]["rawBase64"]
                .as_str()
                .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "missing raw block"))?,
        )?;
        if Digest::of(&bytes) != *evidence {
            return Err(Error::new("SOURCE_INTEGRITY", "raw block digest differs"));
        }
        let block = svm_replay_protocol::parse_json(&bytes)?;
        Ok((bytes, block))
    }
    pub fn transaction(&mut self, signature: &str) -> Result<String, Error> {
        let source=self.read(&json!({"kind":"transaction","genesisHash":self.genesis,"slot":self.target_slot,"signature":signature}))?;
        let wire = source["value"]
            .as_str()
            .ok_or_else(|| Error::new("SOURCE_INTEGRITY", "missing transaction bytes"))?;
        let decoded = transaction::decode(wire)?;
        if decoded["transaction"]["signatures"][0] != signature {
            return Err(Error::new(
                "SOURCE_CONTEXT_MISMATCH",
                "wire signature differs",
            ));
        }
        Ok(wire.into())
    }
}

pub fn source_error(error: SourceError) -> Error {
    Error {
        code: error.code.into(),
        message: error.message,
        details: error.details,
    }
}
