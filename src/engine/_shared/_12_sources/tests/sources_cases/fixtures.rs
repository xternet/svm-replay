use super::*;

pub(super) fn key(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

pub(super) fn hash(bytes: impl AsRef<[u8]>) -> String {
    Digest::of(bytes).as_str().to_owned()
}

pub(super) fn identity(id: &str, bank: bool) -> Value {
    json!({"id":id,"version":"1","genesisHash":key(1),"kind":if bank {"supplied-bank"}else{"captured-history"},
        "coverage":{"firstSlot":0,"lastSlot":1_000_000_000,"completeness":"partial"},
        "capabilities":if bank {json!(["bank-input"])}else{json!(["block","account","transaction"])}})
}

pub(super) fn account(slot: u64, lamports: &str) -> Value {
    json!({"query":{"kind":"account","genesisHash":key(1),"slot":slot,"pubkey":key(2),"phase":"end-slot"},
        "value":{"pubkey":key(2),"sourceSlot":slot,"role":"application","presence":"present","owner":key(0),
            "executable":false,"lamports":lamports,"rentEpoch":"18446744073709551615","dataBase64":""},
        "evidenceHashes":[hash(format!("account-{slot}-{lamports}"))]})
}

pub(super) fn bank() -> Value {
    json!({"query":{"kind":"bank-input","genesisHash":key(1),"slot":12,"parentSlot":10,"blockhash":key(3),
        "blockEvidenceSha256":hash("block"),"executorSourceId":"reviewed-runtime","runtimeProfileId":"profile-1",
        "activeFeatureSetHash":hash("features"),"phase":"post-bank-initialization/pre-transaction","input":"epochStakeEvidence"},
        "value":{"expectedGenesisHash":key(1),"source":{"id":"trusted-producer"},"snapshot":{"complete":true}},
        "evidenceHashes":[hash("trusted-bank-evidence")]})
}

pub(super) struct Capture {
    pub(super) dir: tempfile::TempDir,
    pub(super) manifest: Value,
}

impl Capture {
    pub(super) fn new(records: &[Value], source: Value) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let entries: Vec<_> = records
            .iter()
            .enumerate()
            .map(|(i, record)| {
                let name = format!("record-{i}.json");
                let bytes = serde_json::to_vec(record).expect("serialize test record");
                std::fs::write(dir.path().join(&name), &bytes).expect("write test capture");
                json!({"query":record["query"],"file":name,"sha256":hash(&bytes)})
            })
            .collect();
        Self {
            dir,
            manifest: json!({"schema":"m11-captured-source/v1","identity":source,"entries":entries}),
        }
    }
    pub(super) fn path(&self) -> PathBuf {
        self.dir.path().join("manifest.json")
    }
    pub(super) fn write(&self) -> Digest {
        let bytes = serde_json::to_vec(&self.manifest).expect("serialize manifest");
        std::fs::write(self.path(), &bytes).expect("write test manifest");
        Digest::of(bytes)
    }
    pub(super) fn open(&self) -> Result<CapturedSource, SourceError> {
        CapturedSource::open(&self.path(), &self.write())
    }
    pub(super) fn replace_record(&mut self, bytes: &[u8]) {
        std::fs::write(self.dir.path().join("record-0.json"), bytes)
            .expect("replace exact fixture record");
        self.manifest["entries"][0]["sha256"] = json!(hash(bytes));
    }
}

pub(super) struct ControlledSource {
    pub(super) id: Value,
    pub(super) row: Value,
    pub(super) calls: Arc<AtomicUsize>,
    pub(super) mutated: Arc<AtomicBool>,
    pub(super) mutate: bool,
}

impl HistoricalSource for ControlledSource {
    fn identity(&self) -> Value {
        let mut id = self.id.clone();
        if self.mutated.load(Ordering::SeqCst) {
            id["version"] = json!("changed");
        }
        id
    }
    fn inspect(&self, _: &Value) -> Result<Option<Value>, SourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.mutate {
            self.mutated.store(true, Ordering::SeqCst);
        }
        Ok(Some(self.row.clone()))
    }
}

pub(super) fn controlled(
    id: &str,
    row: Value,
    calls: Arc<AtomicUsize>,
    mutate: bool,
) -> Box<dyn HistoricalSource> {
    Box::new(ControlledSource {
        id: identity(id, false),
        row,
        calls,
        mutated: Arc::new(AtomicBool::new(false)),
        mutate,
    })
}
