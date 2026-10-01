use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use solana_vote_interface::{
    instruction::VoteInstruction,
    state::{TowerSync, Vote, VoteStateUpdate},
};
use svm_replay_engine::shared::bank::slot_hashes::reconstruct;

const EXECUTOR: &str = "litesvm-v0.6.1-agave-2.2.20";
fn hash(slot: u64, domain: &str) -> String {
    bs58::encode(Sha256::digest(format!("{domain}:{slot}"))).into_string()
}
fn vote(slot: u64, bank_hash: &str) -> Value {
    let instruction = VoteInstruction::Vote(Vote::new(vec![slot], bank_hash.parse().unwrap()));
    let data = bs58::encode(bincode::serialize(&instruction).unwrap()).into_string();
    json!({"meta":{"err":null},"transaction":{"message":{
        "accountKeys":["Vote111111111111111111111111111111111111111"],
        "instructions":[{"programIdIndex":0,"data":data}]}}})
}
fn blocks() -> Vec<Value> {
    (1..=514)
        .map(|slot| {
            json!({"slot":slot,"block":{
        "parentSlot":slot-1,"blockhash":hash(slot,"block"),
        "previousBlockhash":hash(slot-1,"block"),
        "transactions":[vote(slot-1,&hash(slot-1,"bank"))]}})
        })
        .collect()
}
fn recover(rows: &[Value]) -> Result<Value, svm_replay_protocol::Error> {
    reconstruct(513, &hash(513, "block"), EXECUTOR, rows)
}
#[test]
fn reviewed_3_1_vote_hashes_keep_same_bank_bytes_and_explicit_runtime_identity() {
    let executor = "litesvm-v0.12.0-agave-3.1.11";
    let proof = reconstruct(513, &hash(513, "block"), executor, &blocks()).unwrap();
    assert_eq!(proof["executorSourceId"], executor);
    assert_eq!(
        proof["dataBase64"],
        recover(&blocks()).unwrap()["dataBase64"]
    );
    svm_replay_engine::shared::bank::slot_hashes::bind_data(&proof, 513).unwrap();
    assert!(reconstruct(
        513,
        &hash(513, "block"),
        "litesvm-v0.12.0-agave-3.1.12",
        &blocks()
    )
    .is_err());
}
#[test]
fn reviewed_3_0_and_4_1_vote_hashes_bind_exact_bytes_without_admitting_other_versions() {
    for executor in [
        "litesvm-v0.8.2-agave-3.0.10",
        "litesvm-v0.14.0-pr402-agave-4.1.2",
    ] {
        let proof = reconstruct(513, &hash(513, "block"), executor, &blocks()).unwrap();
        assert_eq!(proof["executorSourceId"], executor);
        assert_eq!(
            proof["dataBase64"],
            recover(&blocks()).unwrap()["dataBase64"]
        );
        svm_replay_engine::shared::bank::slot_hashes::bind_data(&proof, 513).unwrap();
    }
    for executor in [
        "litesvm-v0.8.2-agave-3.0.11",
        "litesvm-v0.14.0-pr402-agave-4.1.3",
    ] {
        assert!(reconstruct(513, &hash(513, "block"), executor, &blocks()).is_err());
    }
}
#[test]
fn complete_votes_derive_bank_hash_bytes_not_rpc_blockhashes() {
    let proof = recover(&blocks()).unwrap();
    let bytes = STANDARD
        .decode(proof["dataBase64"].as_str().unwrap())
        .unwrap();
    let mut expected = 512u64.to_le_bytes().to_vec();
    for slot in (1..=512u64).rev() {
        expected.extend(slot.to_le_bytes());
        expected.extend(bs58::decode(hash(slot, "bank")).into_vec().unwrap());
    }
    assert_eq!(bytes, expected);
    assert_eq!(proof["sourceSlot"], 513);
    assert!(proof.get("lamports").is_none());
}

#[test]
fn data_only_binding_rejects_metadata_and_malformed_ancestor_bytes() {
    use svm_replay_engine::shared::bank::slot_hashes::bind_data;
    let proof = recover(&blocks()).unwrap();
    let binding = bind_data(&proof, 513).unwrap();
    assert_eq!(binding["dataLen"], 20488);
    assert!(bind_data(&proof, 514).is_err());
    for (name, value) in [
        ("lamports", json!("1")),
        ("scope", json!("full-account")),
        ("executorSourceId", json!("unknown")),
        ("dataSha256", json!("wrong")),
        ("evidence", json!([])),
    ] {
        let mut bad = proof.clone();
        bad[name] = value;
        assert!(bind_data(&bad, 513).is_err(), "{name}");
    }
    let mut bad = proof;
    let mut bytes = STANDARD
        .decode(bad["dataBase64"].as_str().unwrap())
        .unwrap();
    bytes[48..56].copy_from_slice(&512u64.to_le_bytes());
    bad["dataBase64"] = json!(STANDARD.encode(&bytes));
    bad["dataSha256"] = json!(format!("{:x}", Sha256::digest(&bytes)));
    assert!(bind_data(&bad, 513).is_err());
}
#[test]
fn failed_votes_missing_metadata_and_missing_hash_reject() {
    let mut rows = blocks();
    rows[1]["block"]["transactions"][0]["meta"]["err"] =
        json!({"InstructionError":[0,"InvalidArgument"]});
    assert!(recover(&rows).is_err());
    let mut rows = blocks();
    rows[1]["block"]["transactions"][0]["meta"]
        .as_object_mut()
        .unwrap()
        .remove("err");
    assert!(recover(&rows).is_err());
    let mut rows = blocks();
    rows[1]["block"]["transactions"] = json!([]);
    assert!(recover(&rows).is_err());
}
#[test]
fn conflicting_votes_bad_ancestry_and_wrong_target_reject() {
    let mut rows = blocks();
    rows[2]["block"]["transactions"]
        .as_array_mut()
        .unwrap()
        .push(vote(1, &hash(1, "wrong")));
    assert!(recover(&rows).is_err());
    let mut rows = blocks();
    rows[2]["block"]["previousBlockhash"] = json!(hash(100, "wrong"));
    assert!(recover(&rows).is_err());
    assert!(reconstruct(513, &hash(513, "wrong"), EXECUTOR, &blocks()).is_err());
    assert!(reconstruct(513, &hash(513, "block"), "future-runtime", &blocks()).is_err());
}
#[test]
fn skipped_slots_use_parent_chain_not_slot_minus_one() {
    let mut rows = blocks();
    for row in &mut rows {
        let old = row["slot"].as_u64().unwrap();
        row["slot"] = json!(old * 2);
        row["block"]["parentSlot"] = json!((old - 1) * 2);
        row["block"]["transactions"] = json!([vote((old - 1) * 2, &hash((old - 1) * 2, "bank"))]);
    }
    let proof = reconstruct(1026, &hash(513, "block"), EXECUTOR, &rows).unwrap();
    let bytes = STANDARD
        .decode(proof["dataBase64"].as_str().unwrap())
        .unwrap();
    assert_eq!(&bytes[8..16], &1024u64.to_le_bytes());
}

#[test]
fn every_reviewed_vote_wire_variant_recovers_the_same_hash() {
    let h = hash(1, "bank").parse().unwrap();
    let v = Vote::new(vec![1], h);
    let mut state = VoteStateUpdate::from(vec![(1, 1)]);
    state.hash = h;
    let mut tower = TowerSync::from(vec![(1, 1)]);
    tower.hash = h;
    let variants = [
        VoteInstruction::Vote(v.clone()),
        VoteInstruction::VoteSwitch(v, Default::default()),
        VoteInstruction::UpdateVoteState(state.clone()),
        VoteInstruction::UpdateVoteStateSwitch(state.clone(), Default::default()),
        VoteInstruction::CompactUpdateVoteState(state.clone()),
        VoteInstruction::CompactUpdateVoteStateSwitch(state, Default::default()),
        VoteInstruction::TowerSync(tower.clone()),
        VoteInstruction::TowerSyncSwitch(tower, Default::default()),
    ];
    for variant in variants {
        let mut rows = blocks();
        rows[1]["block"]["transactions"][0]["transaction"]["message"]["instructions"][0]["data"] =
            json!(bs58::encode(bincode::serialize(&variant).unwrap()).into_string());
        assert!(recover(&rows).is_ok(), "{variant:?}");
    }
}

#[test]
fn ignored_stream_errors_cannot_produce_a_proof() {
    use svm_replay_engine::shared::bank::slot_hashes::Reconstruction;
    let rows = blocks();
    let mut proof = Reconstruction::new(513, &hash(513, "block"), EXECUTOR).unwrap();
    assert!(proof.observe(0, &rows[0]["block"], EXECUTOR).is_err());
    for row in rows.iter().rev() {
        // A failed stream must not be usable even if the caller retries valid records.
        if proof
            .observe(row["slot"].as_u64().unwrap(), &row["block"], EXECUTOR)
            .is_err()
        {
            assert!(proof.finish().is_err());
            return;
        }
    }
    assert!(proof.finish().is_err());
}
