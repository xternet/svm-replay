use super::*;

/// Resolves every covering source, rejecting disagreement rather than preferring one.
pub struct CompositeSource {
    pub(in super::super::super) sources: Vec<(Box<dyn HistoricalSource>, Value)>,
    pub(in super::super::super) recorded: Vec<Value>,
    pub(in super::super::super) discovery_artifacts: Option<PathBuf>,
}

impl CompositeSource {
    pub fn new(sources: Vec<Box<dyn HistoricalSource>>) -> Result<Self> {
        require(
            !sources.is_empty(),
            "INVALID_SOURCE",
            "at least one source required",
        )?;
        let mut saved = Vec::new();
        let mut networks = BTreeSet::new();
        let mut identities = BTreeSet::new();
        for source in sources {
            let identity = source.identity();
            validate_identity(&identity)?;
            networks.insert(
                text(&identity["genesisHash"], "INVALID_SOURCE", "source genesis")?.to_owned(),
            );
            // Preserve the public composite identity/version format for existing receipts.
            let id = format!(
                "{}:{}",
                text(&identity["id"], "INVALID_SOURCE", "source id")?,
                text(&identity["version"], "INVALID_SOURCE", "source version")?
            );
            identities.insert(id);
            saved.push((source, identity));
        }
        require(
            networks.len() == 1,
            "SOURCE_CONTEXT_MISMATCH",
            "mixed source networks",
        )?;
        require(
            identities.len() == saved.len(),
            "INVALID_SOURCE",
            "duplicate source identity/version",
        )?;
        Ok(Self {
            sources: saved,
            recorded: Vec::new(),
            discovery_artifacts: None,
        })
    }
    pub fn observations(&self) -> Vec<Value> {
        self.recorded.clone()
    }
    pub fn diagnostics(&self) -> Result<Value> {
        let mut sources = Vec::new();
        for (source, identity) in &self.sources {
            require(
                source.identity() == *identity,
                "SOURCE_INTEGRITY",
                "source identity mutated during diagnostics",
            )?;
            let diagnostics = match source.diagnostics() {
                Ok(value) => value,
                Err(error) => json!({"status":"ERROR","error":error}),
            };
            sources.push(json!({"source":identity,"diagnostics":diagnostics}));
        }
        Ok(
            json!({"schema":"svm-source-diagnostics/v1","scope":"source-instance-cumulative","sources":sources}),
        )
    }
    pub fn assert_genesis(&self, expected: &str) -> Result<()> {
        address(
            &json!(expected),
            32,
            "SOURCE_CONTEXT_MISMATCH",
            "expected genesis",
        )?;
        require(
            self.sources
                .iter()
                .all(|(_, identity)| identity["genesisHash"] == expected),
            "SOURCE_CONTEXT_MISMATCH",
            "configured source network differs from request",
        )
    }
    pub(in super::super) fn lookup(
        &mut self,
        query: &Value,
    ) -> Result<(Option<Value>, Vec<Value>)> {
        let key = query_key(query)?;
        let mut observations = Vec::<Value>::new();
        let mut selected = None;
        let mut semantic_hash = None;
        for (source, identity) in &self.sources {
            require(
                source.identity() == *identity,
                "SOURCE_INTEGRITY",
                "source identity mutated",
            )?;
            let record = if covers(identity, query)? {
                let record = source.inspect(query)?;
                require(
                    source.identity() == *identity,
                    "SOURCE_INTEGRITY",
                    "source identity mutated during read",
                )?;
                record
            } else {
                None
            };
            let source_hash = value_hash("m11-source-identity/v1", identity)?;
            let mut observation = json!({"query":query,"queryKey":key,"source":identity,"status":if record.is_some(){"AVAILABLE"}else{"UNAVAILABLE"},"evidenceHashes":[source_hash]});
            if let Some(record) = record {
                validate_record(&record, query)?;
                if let Some(provenance) = record.get("provenance") {
                    observation["provenance"] = provenance.clone();
                }
                let mut semantic = record["value"].clone();
                if query["kind"] == "account" {
                    semantic
                        .as_object_mut()
                        .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "account image"))?
                        .remove("role");
                }
                let digest = value_hash("m11-source-value/v1", &semantic)?;
                observation["valueSha256"] = json!(digest);
                let mut evidence = evidence_hashes(&record)?;
                evidence.push(source_hash.as_str().to_owned());
                observation["evidenceHashes"] = unique_hashes(evidence);
                if semantic_hash
                    .as_ref()
                    .is_some_and(|previous| previous != &digest)
                {
                    let mut ids: Vec<_> = observations
                        .iter()
                        .filter(|o| o["status"] == "AVAILABLE")
                        .map(|o| o["source"]["id"].clone())
                        .collect();
                    ids.push(identity["id"].clone());
                    return Err(SourceError::new(
                        "SOURCE_CONFLICT",
                        "sources disagree on exact input",
                    )
                    .details(json!({"queryKey":key,"sourceIds":ids})));
                }
                semantic_hash = Some(digest);
                if selected.is_none() {
                    selected = Some(record);
                }
            }
            observations.push(observation.clone());
            self.recorded.push(observation);
        }
        let result = if let Some(selected) = selected {
            let mut evidence = Vec::new();
            for observation in &observations {
                evidence.extend(evidence_hashes(observation)?);
            }
            Some(
                json!({"value":selected["value"],"observations":observations,"evidenceHashes":unique_hashes(evidence)}),
            )
        } else {
            None
        };
        Ok((result, observations))
    }
    pub fn inspect(&mut self, query: &Value) -> Result<Option<Value>> {
        Ok(self.lookup(query)?.0)
    }
    pub fn require(&mut self, query: &Value) -> Result<Value> {
        let (result, observations) = self.lookup(query)?;
        if let Some(result) = result {
            return Ok(result);
        }
        Err(
            SourceError::new("SOURCE_UNAVAILABLE", "exact source input unavailable").details(
                json!({"query":query,"queryKey":query_key(query)?,"observations":observations}),
            ),
        )
    }
    pub fn read(&mut self, query: &Value) -> Result<Value> {
        self.require(query)
    }
}
