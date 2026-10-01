use super::*;

pub(super) fn key(n: u8) -> String {
    bs58::encode([n; 32]).into_string()
}

pub(super) fn identity() -> Value {
    json!({"id":"recorded-test","version":"1","kind":"captured-history","genesisHash":key(1),"coverage":{"firstSlot":90,"lastSlot":100,"completeness":"partial"},"capabilities":["account"]})
}

pub(super) fn query(n: u8) -> Value {
    json!({"kind":"account","genesisHash":key(1),"slot":99,"pubkey":key(n),"phase":"end-slot"})
}

pub(super) fn record(q: &Value) -> Value {
    json!({"query":q,"value":{"pubkey":q["pubkey"],"sourceSlot":q["slot"],"presence":"absent","role":"application"},"evidenceHashes":[Digest::of(b"exact captured null")]})
}

pub(super) struct Source {
    pub(super) reads: AtomicUsize,
    pub(super) mode: Mutex<usize>,
    pub(super) identity: Mutex<Value>,
}

impl Source {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            reads: AtomicUsize::new(0),
            mode: Mutex::new(0),
            identity: Mutex::new(identity()),
        })
    }
}

impl HistoricalSource for Source {
    fn identity(&self) -> Value {
        self.identity.lock().unwrap().clone()
    }
    fn inspect(&self, q: &Value) -> Result<Option<Value>, SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        match *self.mode.lock().unwrap() {
            0 => Ok(Some(record(q))),
            1 => Ok(None),
            2 => Err(SourceError::new(
                "SOURCE_RATE_LIMIT",
                "typed test rate limit",
            )),
            3 => {
                let mut r = record(q);
                r["query"]["slot"] = json!(98);
                Ok(Some(r))
            }
            _ => panic!("invalid mode"),
        }
    }
}

pub(super) fn scope(id: &str, pin: bool) -> CacheScope {
    CacheScope {
        namespace_id: id.into(),
        expected_source_identity_sha256: source_identity_sha256(&identity()).unwrap(),
        pin_owner: if pin { Some("case-1".into()) } else { None },
    }
}
