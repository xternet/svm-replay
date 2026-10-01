use super::*;

fn block(n: u8) -> Value {
    let bytes = serde_json::to_vec(&json!({"result":{"parentSlot":10,
        "blockhash":key(n),"transactions":[]}}))
    .unwrap();
    json!({"query":{"kind":"block","genesisHash":key(1),"slot":12,
        "blockEvidenceSha256":hash(&bytes)},"value":{"rawBase64":STANDARD.encode(&bytes)},
        "evidenceHashes":[hash(&bytes)]})
}

#[test]
fn captured_discovery_retains_original_pins_and_records_observations() {
    let row = block(3);
    let capture = Capture::new(&[row.clone()], identity("capture", false));
    let mut source = CompositeSource::new(vec![Box::new(capture.open().unwrap())]).unwrap();
    let found = source.discover_block(12).unwrap();
    assert_eq!(found["value"], row["value"]);
    assert_eq!(source.observations()[0]["query"], row["query"]);
    assert_eq!(
        source.discover_block(13).unwrap_err().code,
        "SOURCE_UNAVAILABLE"
    );
    std::fs::write(capture.dir.path().join("record-0.json"), b"{}").unwrap();
    assert_eq!(
        source.discover_block(12).unwrap_err().code,
        "SOURCE_INTEGRITY"
    );
}

#[test]
fn discovery_rejects_conflicting_captures_and_ambiguous_slot_pins() {
    let a = Capture::new(&[block(3)], identity("a", false));
    let b = Capture::new(&[block(4)], identity("b", false));
    let mut source = CompositeSource::new(vec![
        Box::new(a.open().unwrap()),
        Box::new(b.open().unwrap()),
    ])
    .unwrap();
    assert_eq!(
        source.discover_block(12).unwrap_err().code,
        "SOURCE_CONFLICT"
    );
    let both = Capture::new(&[block(3), block(4)], identity("both", false));
    assert!(both.open().is_err());
}

struct DiscoverySource {
    row: Value,
    change: bool,
    mutated: AtomicBool,
}
impl HistoricalSource for DiscoverySource {
    fn identity(&self) -> Value {
        identity(
            if self.mutated.load(Ordering::SeqCst) {
                "changed"
            } else {
                "source"
            },
            false,
        )
    }
    fn inspect(&self, _: &Value) -> Result<Option<Value>, SourceError> {
        panic!("discovery must not refetch")
    }
    fn discover_block(&self, _: u64) -> Result<Option<Value>, SourceError> {
        if self.row.is_null() {
            return Err(SourceError::new(
                "SOURCE_RPC_ERROR",
                "controlled discovery failure",
            ));
        }
        if self.change {
            self.mutated.store(true, Ordering::SeqCst);
        }
        Ok(Some(self.row.clone()))
    }
}

#[test]
fn discovery_errors_are_retained_for_recovery_attempt_audits() {
    let source = DiscoverySource {
        row: Value::Null,
        change: false,
        mutated: AtomicBool::new(false),
    };
    let mut composite = CompositeSource::new(vec![Box::new(source)]).unwrap();
    assert_eq!(
        composite.discover_block(12).unwrap_err().code,
        "SOURCE_RPC_ERROR"
    );
    assert_eq!(composite.observations()[0]["status"], "ERROR");
    assert_eq!(
        composite.observations()[0]["error"]["code"],
        "SOURCE_RPC_ERROR"
    );
}

#[test]
fn discovery_rejects_wrong_slot_network_and_mutating_identity() {
    for mode in 0..3 {
        let mut row = block(3);
        if mode == 0 {
            row["query"]["slot"] = json!(14);
        }
        if mode == 1 {
            row["query"]["genesisHash"] = json!(key(9));
        }
        let source = DiscoverySource {
            row,
            change: mode == 2,
            mutated: AtomicBool::new(false),
        };
        let mut composite = CompositeSource::new(vec![Box::new(source)]).unwrap();
        assert!(composite.discover_block(12).is_err());
    }
}
