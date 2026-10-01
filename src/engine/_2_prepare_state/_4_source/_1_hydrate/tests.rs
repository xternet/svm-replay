use super::*;
use crate::shared::{
    bank::slot_hashes::SLOT_HASHES,
    runtime::{CancellationToken, ExecutionBudget},
    sources::{CompositeSource, HistoricalSource, SourceError},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use solana_vote_interface::{instruction::VoteInstruction, state::Vote};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
const TARGET: u64 = 357_865_280;
const GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
fn hash() -> String {
    bs58::encode([7u8; 32]).into_string()
}
fn block_hash(slot: u64) -> String {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(&slot.to_le_bytes());
    bs58::encode(bytes).into_string()
}
fn block(slot: u64) -> Vec<u8> {
    let vote = VoteInstruction::Vote(Vote::new(vec![slot - 1], hash().parse().unwrap()));
    serde_json::to_vec(&json!({"result":{"parentSlot":slot-1,"blockhash":block_hash(slot),"previousBlockhash":block_hash(slot-1),
        "transactions":[{"meta":{"err":null,"fee":5000,"loadedAddresses":{"writable":[],"readonly":[]},"innerInstructions":[]},"transaction":{"signatures":["synthetic-signature"],"message":{
            "header":{"numRequiredSignatures":1},"accountKeys":["Vote111111111111111111111111111111111111111"],
            "instructions":[{"programIdIndex":0,"accounts":[],"data":bs58::encode(bincode::serialize(&vote).unwrap()).into_string()}]}}}]}})).unwrap()
}
struct Blocks {
    missing: Option<u64>,
    reads: Arc<AtomicUsize>,
}
impl HistoricalSource for Blocks {
    fn identity(&self) -> Value {
        json!({"id":"synthetic-votes","version":"1","kind":"captured-history","genesisHash":GENESIS,
            "coverage":{"firstSlot":TARGET-600,"lastSlot":TARGET+600,"completeness":"partial"},"capabilities":["account","block"]})
    }
    fn inspect(&self, query: &Value) -> Result<Option<Value>, SourceError> {
        if query["kind"] == "block" {
            return self.discover_block(query["slot"].as_u64().unwrap());
        }
        Ok(Some(
            json!({"query":query,"value":{"pubkey":query["pubkey"],"sourceSlot":query["slot"],
            "role":"sysvar","presence":"absent"},"evidenceHashes":[Digest::of(b"synthetic missing account")]}),
        ))
    }
    fn discover_block(&self, slot: u64) -> Result<Option<Value>, SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.missing == Some(slot) {
            return Ok(None);
        }
        let raw = block(slot);
        Ok(Some(
            json!({"query":{"kind":"block","slot":slot,"genesisHash":GENESIS,"blockEvidenceSha256":Digest::of(&raw)},
            "value":{"rawBase64":STANDARD.encode(&raw)},"evidenceHashes":[Digest::of(&raw)]}),
        ))
    }
}
fn run(missing: Option<u64>, capability: bool) -> (Result<(Value, Vec<Digest>), Error>, usize) {
    let reads = Arc::new(AtomicUsize::new(0));
    let mut sources = CompositeSource::new(vec![Box::new(Blocks {
        missing,
        reads: reads.clone(),
    })])
    .unwrap();
    let budget = ExecutionBudget::new(Duration::from_secs(30), CancellationToken::new()).unwrap();
    let mut history = History::new(&mut sources, GENESIS.into(), TARGET, &budget, 2000).unwrap();
    let raw = block(TARGET);
    let context = crate::shared::bank::context::derive_historical_bank_context(
        &serde_json::from_slice(&raw).unwrap(),
        &json!({"slot":TARGET,"parentSlot":TARGET-1,"blockhash":block_hash(TARGET),"index":0,"signature":"synthetic-signature"}),
        Digest::of(&raw).as_str(), &[]).unwrap();
    let fixture = json!({"target":{"targetSlot":TARGET},"runtime":{"bankContext":context,
        "binding":{"executor":{"id":"litesvm-v0.6.1-agave-2.2.20","m9Build":{"capabilities":
            if capability {json!(["proven-slot-hashes-cache/v1"])} else {json!([])}}}}}});
    let result = hydrate_exact(&mut history, SLOT_HASHES, TARGET, &fixture);
    (result, reads.load(Ordering::SeqCst))
}
#[test]
fn absent_sysvar_recovers_data_through_budgeted_history() {
    let (result, reads) = run(None, true);
    let (proof, evidence) = result.unwrap();
    assert_eq!(proof["scope"], "sysvar-data-only");
    assert_eq!(proof["dataLen"], 20488);
    assert_eq!(reads, 546);
    assert_eq!(proof["targetBlockhash"], block_hash(TARGET));
    assert_ne!(proof["targetBlockhash"], block_hash(TARGET - 1));
    assert!(evidence.len() >= 545);
    assert!(proof.get("lamports").is_none());
}
#[test]
fn unavailable_ancestor_and_old_worker_do_not_produce_proof() {
    let (result, _) = run(Some(TARGET - 30), true);
    assert_eq!(result.unwrap_err().code, "SOURCE_UNAVAILABLE");
    let (result, reads) = run(None, false);
    assert_eq!(result.unwrap_err().code, "UNSUPPORTED_RUNTIME_CAPABILITY");
    assert_eq!(reads, 0);
}

#[test]
fn missing_optional_later_observer_does_not_fake_or_skip_an_ancestor() {
    let (result, reads) = run(Some(TARGET + 32), true);
    let (proof, _) = result.unwrap();
    assert_eq!(reads, 515);
    assert_eq!(proof["evidence"][0]["slot"], TARGET);
    assert_eq!(proof["evidence"].as_array().unwrap().len(), 513);
}
