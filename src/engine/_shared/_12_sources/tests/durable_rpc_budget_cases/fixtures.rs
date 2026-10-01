use super::*;

pub(super) fn key(n: u8) -> String {
    bs58::encode([n; 32]).into_string()
}

pub(super) fn config() -> AlchemyConfig {
    AlchemyConfig {
        id: "recorded-provider".into(),
        version: "1".into(),
        expected_genesis_hash: key(1),
        first_slot: 90,
        last_slot: 100,
    }
}

pub(super) fn limits() -> SourceLimits {
    SourceLimits {
        max_requests: 10,
        max_account_reads: 5,
        max_download_bytes: 8192,
        max_response_bytes: 1024,
        deadline: Duration::from_secs(5),
    }
}

pub(super) fn query(n: u8) -> Value {
    json!({"kind":"account","genesisHash":key(1),"slot":99,"pubkey":key(n),"phase":"end-slot"})
}

pub(super) fn response(value: Value) -> Result<RpcHttpResponse, TransportFailure> {
    Ok(RpcHttpResponse {
        status: 200,
        body: serde_json::to_vec(&value).unwrap(),
    })
}

pub(super) fn genesis() -> Result<RpcHttpResponse, TransportFailure> {
    response(json!({"jsonrpc":"2.0","id":1,"result":key(1)}))
}

pub(super) fn account() -> Result<RpcHttpResponse, TransportFailure> {
    response(json!([{"jsonrpc":"2.0","id":0,"result":{"context":{"slot":99},"value":null}}]))
}

pub(super) fn inspect(root: &Path, id: &str) -> Value {
    Store::open(root)
        .unwrap()
        .execute(json!({"version":1,"op":"budget","action":"inspect","id":id}))
        .unwrap()
}

pub(super) struct Transport {
    pub(super) root: PathBuf,
    pub(super) id: String,
    pub(super) responses: Mutex<VecDeque<Result<RpcHttpResponse, TransportFailure>>>,
    pub(super) observed: Mutex<Vec<Value>>,
    pub(super) calls: AtomicUsize,
}

impl Transport {
    pub(super) fn new(
        root: &Path,
        responses: Vec<Result<RpcHttpResponse, TransportFailure>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            root: root.to_owned(),
            id: "campaign-1".into(),
            responses: Mutex::new(responses.into()),
            observed: Mutex::new(vec![]),
            calls: AtomicUsize::new(0),
        })
    }
}

impl RpcTransport for Transport {
    fn post(&self, _: &[u8], _: Duration, bound: u64) -> Result<RpcHttpResponse, TransportFailure> {
        let mut persisted = inspect(&self.root, &self.id);
        // A wholly separate SQLite handle observes the committed reservation.
        assert!(persisted["used"]["requests"].as_u64().unwrap() >= 1);
        assert!(persisted["used"]["bytes"].as_u64().unwrap() >= bound);
        persisted["transportBound"] = json!(bound);
        self.observed.lock().unwrap().push(persisted);
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("unbudgeted/unexpected RPC")
    }
}

pub(super) fn budget(root: &Path, limits: &SourceLimits) -> Arc<DurableRpcBudget> {
    Arc::new(
        DurableRpcBudget::open(Store::open(root).unwrap(), "campaign-1".into(), limits).unwrap(),
    )
}

pub(super) fn source(
    limits: SourceLimits,
    transport: Arc<dyn RpcTransport>,
    ledger: Arc<DurableRpcBudget>,
) -> AlchemySource {
    AlchemySource::with_transport(config(), limits, transport)
        .unwrap()
        .with_budget(ledger)
        .unwrap()
}
