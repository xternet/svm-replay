// Shared synthetic fixture for engine and CLI ownership/cache tests.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};
use svm_replay_engine::{
    _1_resolve_runtime::ResolvedWorker,
    shared::runtime::{file_sha256, WorkerSpec},
};
use svm_replay_protocol::{transaction, worker::WorkerDescriptor, Digest, PreparedRequest};

pub fn fixture(root: &Path, worker_source: &str) -> (PreparedRequest, ResolvedWorker) {
    let path = root.join("worker");
    fs::write(&path, worker_source).expect("test worker source");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("worker executable");
    let worker_hash = file_sha256(&path).expect("actual test worker hash");
    let source_hash = Digest::of(worker_source);
    let build_hash = Digest::of(b"synthetic fee ledger build");
    let capabilities = json!([
        "guarded-generic-sysvars/v1",
        "historical-boundary-checkpoint/v1"
    ]);
    let descriptor: WorkerDescriptor = serde_json::from_value(json!({"family":"v2-3","file":"worker","sha256":worker_hash,"buildHash":build_hash,
        "sourceSha256":source_hash,"executorSourceId":"synthetic-ledger","capabilities":capabilities})).expect("descriptor");
    let mut transactions = Vec::new();
    let mut encoded = Vec::new();
    let mut signatures = Vec::new();
    for index in 0..2 {
        let mut wire = vec![1];
        wire.extend([7 + index; 64]);
        wire.extend([1, 0, 0, 1]);
        wire.extend([1; 32]);
        wire.extend([3; 32]);
        wire.push(0);
        let bytes = STANDARD.encode(wire);
        let mut decoded = transaction::decode(&bytes).expect("synthetic wire");
        signatures.push(decoded["transaction"]["signatures"][0].clone());
        decoded["meta"] = json!({"err":null,"innerInstructions":[],"logMessages":[],"computeUnitsConsumed":0,"fee":5,
            "preBalances":[1000 - u64::from(index)*5],"postBalances":[995-u64::from(index)*5],"preTokenBalances":[],"postTokenBalances":[]});
        transactions.push(decoded);
        encoded.push(bytes);
    }
    let payer = bs58::encode([1; 32]).into_string();
    let block = json!({"result":{"parentSlot":19,"blockhash":bs58::encode([3;32]).into_string(),"transactions":transactions}});
    let raw_block = serde_json::to_vec(&block).expect("raw test archive");
    let block_hash = Digest::of(&raw_block);
    let signature = signatures[1].as_str().expect("target signature");
    let account = json!({"pubkey":payer,"sourceSlot":19,"role":"application","presence":"present","owner":"11111111111111111111111111111111","executable":false,"lamports":"1000","rentEpoch":"0","dataBase64":""});
    let mut end = account.clone();
    end["sourceSlot"] = json!(20);
    end["lamports"] = json!("990");
    let mut request:PreparedRequest = serde_json::from_value(json!({"schema":"svm-replay-prepared/v1","requestId":"cache-test","family":"v2-3",
        "candidate":{"id":"cache-test","slot":20,"transactionIndex":1,"targetSignature":signature,"blockSourceHash":block_hash,
            "rawEvidenceHash":Digest::of(format!("{block_hash_string}\n1\n{signature}\n",block_hash_string=block_hash.as_str())),"runtimeProfileId":"toy-profile","executorSourceId":"synthetic-ledger"},
        "fixture":{"schema":"svm-simulate-m6-fixture/v1","caseId":"cache-test","target":{"targetSlot":20,"parentSlot":19,"index":1,"signature":signature,
            "prefixIndices":[0],"prefixSignatures":[signatures[0]],"prefixTransactionsBase64":[encoded[0]],"transactionBase64":encoded[1],"replacementTransactionBase64":null,
            "evidenceAccounts":[payer],"omittedFeeEffects":[]},"accounts":[account],"endAccounts":[end],"accountOverride":null,
            "runtime":{"bankContext":{"requiredRuntimeSysvars":[],"lamportsPerSignature":"5"},"binding":{"targetSlot":20,"runtimeProfileId":"toy-profile","bindingHash":"toy-binding",
                "executor":{"id":"synthetic-ledger","sourceEvidenceHash":source_hash,"m9Build":{"binarySha256":worker_hash,"buildHash":build_hash,"capabilities":capabilities}}}}},
        "rawBlockBase64":STANDARD.encode(&raw_block),"blockSha256":block_hash,"sourceEvidenceHashes":[block_hash],"metadataPolicy":"STRICT",
        "limits":{"timeoutMs":10000,"maxOutputBytes":1048576,"maxDiagnosticBytes":4096}})).expect("cache request");
    let profile = Digest::of(b"synthetic fee-ledger profile");
    request.candidate["runtimeProfileId"] = json!(profile);
    let binding = &mut request.fixture["runtime"]["binding"];
    binding["runtimeProfileId"] = json!(profile);
    binding["schema"] = json!("svm-simulate-m6-runtime-binding/v1");
    binding["features"] = json!([]);
    binding["activeFeatureSetHash"] = json!(Digest::of(b"svm-feature-set/v1\n\n"));
    binding["enabledBuiltinsHash"] = json!(Digest::of(b"synthetic builtins"));
    binding["enabledPrecompilesHash"] = json!(Digest::of(b"synthetic precompiles"));
    binding.as_object_mut().unwrap().remove("bindingHash");
    binding["bindingHash"] = json!(Digest::of(format!(
        "m6-runtime-binding/v1\n{}\n",
        serde_json::to_string(binding).unwrap()
    )));
    (
        request,
        ResolvedWorker {
            descriptor,
            spec: WorkerSpec {
                executable: path,
                sha256: worker_hash,
            },
        },
    )
}
