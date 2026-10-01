use super::*;

/// Stream full blocks one at a time; keep only hashes and provenance, not block bodies.
pub struct Reconstruction {
    target: u64,
    target_hash: String,
    executor: String,
    next: Option<(u64, String)>,
    seen_target: bool,
    ancestors: Vec<u64>,
    hashes: BTreeMap<u64, String>,
    evidence: Vec<Value>,
    invalid: bool,
}

impl Reconstruction {
    pub fn new(target: u64, target_hash: &str, executor: &str) -> Result<Self> {
        reviewed(executor)?;
        pubkey(&json!(target_hash))?;
        Ok(Self {
            target,
            target_hash: target_hash.into(),
            executor: executor.into(),
            next: None,
            seen_target: false,
            ancestors: Vec::new(),
            hashes: BTreeMap::new(),
            evidence: Vec::new(),
            invalid: false,
        })
    }

    /// The caller must bind each finalized raw block and executor to this slot/network.
    pub fn observe(&mut self, slot: u64, block: &Value, executor: &str) -> Result<()> {
        check(!self.invalid, "vote-hash proof stream already failed")?;
        self.invalid = true;
        self.observe_checked(slot, block, executor)?;
        self.invalid = false;
        Ok(())
    }

    fn observe_checked(&mut self, slot: u64, block: &Value, executor: &str) -> Result<()> {
        reviewed(executor)?;
        check(self.ancestors.len() < ENTRIES, "excess ancestor block")?;
        check(
            self.evidence.len() <= ENTRIES * 2,
            "observer-chain proof bound exceeded",
        )?;
        let block_hash = pubkey(field(block, "blockhash")?)?;
        if let Some((expected_slot, expected_hash)) = &self.next {
            check(
                slot == *expected_slot && block_hash == expected_hash,
                "ancestor link differs",
            )?;
        } else {
            check(slot >= self.target, "first observer precedes target")?;
        }
        if slot == self.target {
            check(block_hash == self.target_hash, "target block hash differs")?;
            self.seen_target = true;
        } else if self.seen_target {
            self.ancestors.push(slot);
        }
        let parent = integer(field(block, "parentSlot")?)?;
        check(parent < slot, "nondecreasing ancestor slot")?;
        let previous = pubkey(field(block, "previousBlockhash")?)?;
        for (voted, bank_hash) in _0_votes::witnesses(block, slot)? {
            if voted >= self.target {
                continue;
            }
            if let Some(known) = self.hashes.insert(voted, bank_hash.clone()) {
                check(known == bank_hash, "conflicting successful vote hashes")?;
            }
        }
        self.evidence.push(
            json!({"slot":slot,"blockSha256":hash(canonical_json(block)),
            "executorSourceId":executor}),
        );
        self.next = Some((parent, previous.into()));
        Ok(())
    }

    pub fn finish(self) -> Result<Value> {
        check(!self.invalid, "vote-hash proof stream already failed")?;
        check(
            self.seen_target && self.ancestors.len() == ENTRIES,
            "incomplete ancestor chain",
        )?;
        let mut data = (ENTRIES as u64).to_le_bytes().to_vec();
        let mut missing = Vec::new();
        for slot in self.ancestors {
            if let Some(value) = self.hashes.get(&slot) {
                data.extend(slot.to_le_bytes());
                data.extend(
                    bs58::decode(value)
                        .into_vec()
                        .map_err(|e| fail(e.to_string()))?,
                );
            } else {
                missing.push(slot);
            }
        }
        if !missing.is_empty() {
            return Err(Error::new(
                "UNSUPPORTED_SLOT_HASHES_EVIDENCE",
                "successful votes do not cover every required ancestor",
            )
            .with_details(json!({"missingSlots":missing})));
        }
        Ok(
            json!({"schema":"vote-derived-slot-hashes/v1","pubkey":SLOT_HASHES,
            "sourceSlot":self.target,"targetBlockhash":self.target_hash,"executorSourceId":self.executor,
            "dataBase64":STANDARD.encode(&data),"dataSha256":hash(&data),"dataLen":data.len(),
            "evidence":self.evidence,"scope":"sysvar-data-only"}),
        )
    }
}

/// Convenience for already supplied records; acquisition should use the streaming API.
pub fn reconstruct(
    target: u64,
    target_hash: &str,
    executor: &str,
    rows: &[Value],
) -> Result<Value> {
    let mut ordered = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        let slot = integer(field(row, "slot")?)?;
        check(seen.insert(slot), "duplicate block slot")?;
        ordered.insert(slot, field(row, "block")?);
    }
    let mut proof = Reconstruction::new(target, target_hash, executor)?;
    for (slot, block) in ordered.into_iter().rev() {
        proof.observe(slot, block, executor)?;
    }
    proof.finish()
}
