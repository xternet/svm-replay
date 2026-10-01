use super::*;

pub(super) fn key(n: u8) -> String {
    bs58::encode([n; 32]).into_string()
}

pub(super) fn config() -> AlchemyConfig {
    AlchemyConfig {
        id: "approved-historical".into(),
        version: "1".into(),
        expected_genesis_hash: key(1),
        first_slot: 90,
        last_slot: 100,
    }
}

pub(super) fn limits() -> SourceLimits {
    SourceLimits {
        max_requests: 20,
        max_account_reads: 10,
        max_download_bytes: 1_000_000,
        max_response_bytes: 100_000,
        deadline: Duration::from_secs(5),
    }
}

pub(super) fn query(n: u8) -> Value {
    json!({"kind":"account","genesisHash":key(1),"slot":99,"pubkey":key(n),"phase":"end-slot"})
}

pub(super) fn genesis() -> Value {
    json!({"jsonrpc":"2.0","id":1,"result":key(1)})
}

pub(super) fn account() -> Value {
    json!([{"jsonrpc":"2.0","id":0,"result":{"context":{"slot":99},"value":{"lamports":9007199254740993u64,"rentEpoch":u64::MAX,"owner":key(0),"executable":false,"data":["AQID","base64"]}}}])
}

pub(super) struct Transport {
    pub(super) responses: Mutex<VecDeque<Result<RpcHttpResponse, TransportFailure>>>,
    pub(super) requests: Mutex<Vec<Value>>,
}

impl Transport {
    pub(super) fn values(values: Vec<Value>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(
                values
                    .into_iter()
                    .map(|v| {
                        Ok(RpcHttpResponse {
                            status: 200,
                            body: serde_json::to_vec(&v).unwrap(),
                        })
                    })
                    .collect(),
            ),
            requests: Mutex::new(vec![]),
        })
    }
}

impl RpcTransport for Transport {
    fn post(
        &self,
        request: &[u8],
        timeout: Duration,
        max_response_bytes: u64,
    ) -> Result<RpcHttpResponse, TransportFailure> {
        assert!(!timeout.is_zero());
        assert!(max_response_bytes > 0);
        self.requests
            .lock()
            .unwrap()
            .push(svm_replay_protocol::parse_json(request).unwrap());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected RPC")
    }
}
