use super::*;

impl CompositeSource {
    /// Discover and pin finalized history without weakening pinned-query reads.
    /// Every covering source participates; contradictory observations reject.
    pub fn discover_block(&mut self, target: u64) -> Result<Value> {
        slot(&json!(target), "INVALID_QUERY", "discovery slot")?;
        let mut selected = None;
        let mut evidence = Vec::new();
        let mut observations = Vec::new();
        for (source, identity) in &self.sources {
            require(
                source.identity() == *identity,
                "SOURCE_INTEGRITY",
                "source identity mutated",
            )?;
            let discovery =
                json!({"kind":"block","slot":target,"genesisHash":identity["genesisHash"]});
            let record = if covers(identity, &discovery)? {
                match source.discover_block(target) {
                    Ok(record) => record,
                    Err(error) => {
                        self.recorded
                            .push(json!({"discovery":discovery,"source":identity,
                            "status":"ERROR","error":error}));
                        return Err(error);
                    }
                }
            } else {
                None
            };
            require(
                source.identity() == *identity,
                "SOURCE_INTEGRITY",
                "source identity mutated during discovery",
            )?;
            let identity_hash = value_hash("m11-source-identity/v1", identity)?;
            let mut observation = json!({"discovery":discovery,"source":identity,
                "status":"UNAVAILABLE","evidenceHashes":[identity_hash]});
            if let Some(record) = record {
                let query = &record["query"];
                require(
                    query["kind"] == "block"
                        && query["slot"] == target
                        && query["genesisHash"] == identity["genesisHash"],
                    "SOURCE_CONTEXT_MISMATCH",
                    "discovered block boundary differs",
                )?;
                validate_record(&record, query)?;
                let mut hashes = evidence_hashes(&record)?;
                hashes.push(identity_hash.as_str().to_owned());
                observation = json!({"query":query,"queryKey":query_key(query)?,"source":identity,
                    "status":"AVAILABLE","evidenceHashes":unique_hashes(hashes.clone()),
                    "valueSha256":value_hash("m11-source-value/v1", &record["value"])?});
                if let Some(provenance) = record.get("provenance") {
                    observation["provenance"] = provenance.clone();
                }
                self.retain_discovery_provenance(&mut observation)?;
                self.recorded.push(observation.clone());
                observations.push(observation);
                if selected
                    .as_ref()
                    .is_some_and(|value| value != &record["value"])
                {
                    return Err(SourceError::new(
                        "SOURCE_CONFLICT",
                        "sources disagree on discovered block",
                    )
                    .details(json!({"slot":target,"observations":observations})));
                }
                selected = Some(record["value"].clone());
                evidence.extend(hashes);
            } else {
                self.recorded.push(observation.clone());
                observations.push(observation);
            }
        }
        match selected {
            Some(value) => Ok(
                json!({"value":value,"observations":observations,"evidenceHashes":unique_hashes(evidence)}),
            ),
            None => Err(SourceError::new(
                "SOURCE_UNAVAILABLE",
                "historical block discovery unavailable",
            )
            .details(json!({"slot":target,"observations":observations}))),
        }
    }
}
