use super::*;

pub(super) fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

pub(super) fn key(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

pub(super) fn seal(bound: &mut Value, domain: &str, kind: &str) {
    let object = bound.as_object_mut().unwrap();
    object.remove("source");
    object.remove("proofHash");
    let snapshot = Value::Object(object.clone());
    let source = json!({"id":"synthetic-bank-producer","kind":kind,"evidenceSha256":hash(canonical_json(&snapshot))});
    let proof = hash(format!(
        "{domain}\n{}\n",
        canonical_json(&json!({"snapshot":snapshot,"source":source}))
    ));
    bound["source"] = source;
    bound["proofHash"] = json!(proof);
}

pub(super) fn reseal(fixture: &mut Value) {
    seal(
        &mut fixture["runtime"]["epochStakes"],
        "svm-bound-epoch-stakes/v1",
        "controlled-current-bank-epoch-snapshot/v1",
    );
    seal(
        &mut fixture["runtime"]["initializedStakeSnapshot"],
        "svm-bound-initialized-stakes/v1",
        "controlled-bank-initialization/v1",
    );
}

pub(super) fn fixture() -> Value {
    let slot = 20u64;
    let parent = 19u64;
    let epoch = 4u64;
    let mut clock = [0u8; 40];
    clock[0..8].copy_from_slice(&slot.to_le_bytes());
    clock[16..24].copy_from_slice(&epoch.to_le_bytes());
    let block_hash = hash(b"synthetic archived raw block");
    let feature_hash = hash(b"synthetic exact features");
    let blockhash = key(8);
    let previous = key(9);
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Lineage<'a> {
        source_hash: &'a str,
        target_slot: u64,
        parent_slot: u64,
        blockhash: &'a str,
        previous_blockhash: &'a str,
    }
    let lineage = serde_json::to_vec(&Lineage {
        source_hash: &block_hash,
        target_slot: slot,
        parent_slot: parent,
        blockhash: &blockhash,
        previous_blockhash: &previous,
    })
    .unwrap();
    let parent_account = json!({"pubkey":key(3),"sourceSlot":parent,"role":"application","presence":"present",
        "owner":"Stake11111111111111111111111111111111111111","executable":false,"lamports":"100",
        "rentEpoch":"18446744073709551615","dataBase64":"AQ=="});
    let mut initialized = parent_account.clone();
    initialized["sourceSlot"] = json!(slot);
    initialized["dataBase64"] = json!("Ag==");
    let mut fixture = json!({"target":{"targetSlot":slot,"parentSlot":parent},
        "clock":{"pubkey":"SysvarC1ock11111111111111111111111111111111","presence":"present","sourceSlot":slot,
            "role":"sysvar","executable":false,"owner":"Sysvar1111111111111111111111111111111111111","dataBase64":STANDARD.encode(clock)},
        "accounts":[initialized],
        "runtime":{"clockDataHash":hash(clock),"binding":{"targetSlot":slot,"executor":{"id":"worker",
            "m9Build":{"capabilities":["guarded-epoch-stakes/v1"]}},"runtimeProfileId":"profile","activeFeatureSetHash":feature_hash},
            "bankContext":{"targetSlot":slot,"parentSlot":parent,"blockSourceHash":block_hash,"executionBlockhash":previous,
                "blockLineageHash":hash(lineage)},
            "epochStakes":{"schema":"svm-current-bank-epoch-stakes/v1","genesisHash":key(1),"slot":slot,"parentSlot":parent,
                "blockhash":blockhash,"blockEvidenceSha256":block_hash,"executorSourceId":"worker","runtimeProfileId":"profile",
                "activeFeatureSetHash":feature_hash,"clockDataSha256":hash(clock),"complete":true,"epoch":epoch.to_string(),
                "totalStake":"7","voteStakes":[{"voteAccount":key(2),"stake":"7"}]},
            "initializedStakeSnapshot":{"schema":"svm-bank-initialized-stakes/v1","genesisHash":key(1),"slot":slot,"parentSlot":parent,
                "blockhash":blockhash,"blockEvidenceSha256":block_hash,"executorSourceId":"worker","runtimeProfileId":"profile",
                "activeFeatureSetHash":feature_hash,"phase":"post-bank-initialization/pre-transaction",
                "accounts":[{"parent":parent_account,"initialized":initialized}]}}});
    reseal(&mut fixture);
    fixture
}

pub(super) fn evidence(bound: &Value) -> Value {
    let mut snapshot = bound.clone();
    snapshot.as_object_mut().unwrap().remove("source");
    snapshot.as_object_mut().unwrap().remove("proofHash");
    json!({"expectedGenesisHash":bound["genesisHash"],"source":bound["source"],"snapshot":snapshot})
}

pub(super) fn initialized_request(fixture: &Value) -> Value {
    let bound = &fixture["runtime"]["initializedStakeSnapshot"];
    json!({"accounts":bound["accounts"].as_array().unwrap().iter().map(|v|v["parent"].clone()).collect::<Vec<_>>(),
        "slot":bound["slot"],"parentSlot":bound["parentSlot"],"blockhash":bound["blockhash"],
        "blockSourceHash":bound["blockEvidenceSha256"],"runtime":fixture["runtime"]["binding"],"evidence":evidence(bound)})
}

pub(super) fn discovery_fixture() -> Value {
    let mut value = fixture();
    let clock = &value["clock"];
    let bytes = STANDARD
        .decode(clock["dataBase64"].as_str().unwrap())
        .unwrap();
    value["runtime"]["genericSysvars"] = json!([{"pubkey":clock["pubkey"],"sourceSlot":20,
        "dataSha256":hash(&bytes),"dataLen":bytes.len()}]);
    value["runtime"]["bankContext"]["requiredRuntimeSysvars"] = json!([
        {"pubkey":value["clock"]["pubkey"],"requirement":"tracked-generic-sysvar-context-v1"}]);
    value
}
