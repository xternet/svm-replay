use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};

const SLOT: u64 = 100;
const MEMO: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";

pub(super) fn key(seed: u8) -> [u8; 32] {
    [seed; 32]
}

/// One archived legacy transaction signed by `seed` over `keys`, invoking the last key with
/// `data`, exactly as SVM Replay's codec reads it back from the wire.
pub(super) fn entry(seed: u8, keys: &[[u8; 32]], data: &[u8]) -> Value {
    let mut bytes = vec![1];
    bytes.extend([seed; 64]);
    bytes.extend([1, 0, 1, keys.len() as u8]);
    keys.iter().for_each(|key| bytes.extend(key));
    bytes.extend([9; 32]);
    bytes.extend([1, keys.len() as u8 - 1, 1, 0, data.len() as u8]);
    bytes.extend(data);
    let mut entry = svm_replay_protocol::transaction::decode(&STANDARD.encode(&bytes)).unwrap();
    entry["meta"] = json!({"err":null,"loadedAddresses":{"writable":[],"readonly":[]}});
    entry
}

pub(super) fn sig(seed: u8) -> String {
    bs58::encode([seed; 64]).into_string()
}

pub(super) fn address() -> String {
    bs58::encode(key(2)).into_string()
}

/// A boundary block of four transactions at slot 100 whose fork position is 2: s1 and s2
/// happened on the fork, s3 and s4 did not. The address is in s1, s3 and s4; s1 carries a memo.
pub(super) struct History {
    pub(super) block: Value,
    pub(super) position: u64,
    answers: Vec<(&'static str, Value)>,
    queued: Vec<(&'static str, Value)>,
    blocks: Vec<(u64, Value)>,
    pub(super) reads: Vec<Value>,
}

impl History {
    pub(super) fn new() -> Self {
        let memo: [u8; 32] = bs58::decode(MEMO).into_vec().unwrap().try_into().unwrap();
        let block = json!({"parentSlot":99,"previousBlockhash":bs58::encode([8; 32]).into_string(),
            "blockhash":bs58::encode([7; 32]).into_string(),"blockHeight":90,"blockTime":1000,
            "transactions":[entry(1, &[key(1), key(2), memo], b"hi"), entry(2, &[key(5), key(6)], b"x"),
                entry(3, &[key(3), key(2)], b"y"), entry(4, &[key(4), key(2)], b"z")]});
        Self {
            block,
            position: 2,
            answers: vec![],
            queued: vec![],
            blocks: vec![],
            reads: vec![],
        }
    }
    pub(super) fn answer(mut self, kind: &'static str, value: Value) -> Self {
        self.answers.push((kind, value));
        self
    }
    /// One answer for the next read of `kind`, before any standing answer: pages in order.
    pub(super) fn then(mut self, kind: &'static str, value: Value) -> Self {
        self.queued.push((kind, value));
        self
    }
    /// An archived block the stand-in serves; any other block is offline.
    pub(super) fn archived(mut self, slot: u64, block: Value) -> Self {
        self.blocks.push((slot, block));
        self
    }
}

impl Archive for History {
    fn boundary_block(&self) -> Result<(std::sync::Arc<Value>, u64, u64), Error> {
        Ok((std::sync::Arc::new(self.block.clone()), SLOT, self.position))
    }
    fn archive(&mut self, query: &Value) -> Result<Value, Error> {
        self.reads.push(query.clone());
        let kind = query["kind"].as_str().unwrap();
        if let Some(at) = self.queued.iter().position(|(queued, _)| *queued == kind) {
            return Ok(self.queued.remove(at).1);
        }
        self.answers
            .iter()
            .find(|(answer, _)| *answer == kind)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| Error::new("SOURCE_TRANSPORT", "the archive is offline"))
    }
    fn archived_block(&mut self, slot: u64) -> Result<Value, Error> {
        self.reads.push(json!({"block": slot}));
        self.blocks
            .iter()
            .find(|(at, _)| *at == slot)
            .map(|(_, block)| block.clone())
            .ok_or_else(|| Error::new("SOURCE_TRANSPORT", "the archive is offline"))
    }
}

pub(super) fn read(history: &mut History, query: Value) -> Result<Value, Error> {
    _6_archive::read(history, serde_json::from_value(query).unwrap())
}
