use super::*;
use serde_json::Value;
use solana_vote_interface::{instruction::VoteInstruction, state::Vote};
use svm_replay_engine::shared::bank::slot_hashes::{reconstruct, SLOT_HASHES};

fn inputs() -> (Value, Value) {
    let hash = bs58::encode([7u8; 32]).into_string();
    let block_hash = |slot: u64| {
        let mut bytes = [0; 32];
        bytes[..8].copy_from_slice(&slot.to_le_bytes());
        bs58::encode(bytes).into_string()
    };
    let rows: Vec<_> = (1..=513u64).map(|slot| {
        let vote = VoteInstruction::Vote(Vote::new(vec![slot-1], hash.parse().unwrap()));
        json!({"slot":slot,"block":{"parentSlot":slot-1,"blockhash":block_hash(slot),"previousBlockhash":block_hash(slot-1),
            "transactions":[{"meta":{"err":null,"fee":5000,"loadedAddresses":{"writable":[],"readonly":[]},"innerInstructions":[]},"transaction":{"signatures":["synthetic-signature"],"message":{
                "header":{"numRequiredSignatures":1},
                "accountKeys":["Vote111111111111111111111111111111111111111"],
                "instructions":[{"programIdIndex":0,"accounts":[],"data":bs58::encode(bincode::serialize(&vote).unwrap()).into_string()}]
            }}}]}})
    }).collect();
    let executor = "litesvm-v0.6.1-agave-2.2.20";
    let proof = reconstruct(513, &block_hash(513), executor, &rows).unwrap();
    let mut base = fixture();
    base["target"]["targetSlot"] = json!(513);
    base["target"]["parentSlot"] = json!(512);
    base["clock"]["sourceSlot"] = json!(513);
    let mut bytes = [0u8; 40];
    bytes[..8].copy_from_slice(&513u64.to_le_bytes());
    base["clock"]["dataBase64"] = json!(STANDARD.encode(bytes));
    base["runtime"]["bankContext"] = svm_replay_engine::shared::bank::context::derive_historical_bank_context(
        &json!({"result":rows[512]["block"]}),
        &json!({"slot":513,"parentSlot":512,"blockhash":block_hash(513),"index":0,"signature":"synthetic-signature"}),
        svm_replay_protocol::Digest::of(b"synthetic block source").as_str(), &[]).unwrap();
    base["runtime"]["binding"]["executor"] = json!({"id":executor});
    (base, proof)
}

#[test]
fn data_only_discovery_roundtrips_without_an_account_image() {
    let (base, proof) = inputs();
    let mut exact = initial_inputs(&base).unwrap();
    add_input(&mut exact, SLOT_HASHES, proof.clone(), 513).unwrap();
    let tracked = tracked_fixture(&base, &exact).unwrap();
    assert_eq!(tracked["accounts"], json!([]));
    assert_eq!(tracked["runtime"]["slotHashesData"], proof);
    assert!(available_generic_sysvars(&tracked)
        .unwrap()
        .contains(SLOT_HASHES));
    assert_eq!(
        tracked_fixture(&tracked, &initial_inputs(&tracked).unwrap()).unwrap(),
        tracked
    );
    let original = tracked_fixture(&base, &initial_inputs(&base).unwrap()).unwrap();
    assert_ne!(
        tracked["runtime"]["profileHash"],
        original["runtime"]["profileHash"]
    );
}

#[test]
fn data_only_discovery_rejects_conflicting_account_and_context() {
    let (base, proof) = inputs();
    let mut exact = initial_inputs(&base).unwrap();
    add_input(&mut exact, SLOT_HASHES, proof, 513).unwrap();
    for name in ["accounts", "endAccounts"] {
        let mut bad = base.clone();
        bad[name] = json!([{"pubkey":SLOT_HASHES}]);
        assert!(tracked_fixture(&bad, &exact).is_err(), "{name}");
    }
    let mut bad = base.clone();
    bad["runtime"]["binding"]["executor"]["id"] = json!("other-runtime");
    assert!(tracked_fixture(&bad, &exact).is_err());
    let mut tracked = tracked_fixture(&base, &exact).unwrap();
    tracked["runtime"]["bankContext"]["executionBlockhash"] =
        json!(bs58::encode([8u8; 32]).into_string());
    assert!(available_generic_sysvars(&tracked).is_err());
}
