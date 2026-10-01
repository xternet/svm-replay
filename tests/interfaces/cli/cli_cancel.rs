#![cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use svm_replay_engine::shared::runtime::file_sha256;
use svm_replay_protocol::{transaction, Digest};

fn write_json(path: &Path, value: &Value) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize test input");
    fs::write(path, &bytes).expect("write test input");
    Digest::of(bytes).as_str().into()
}

fn prepare(root: &Path) -> (String, std::path::PathBuf) {
    let pid_file = root.join("worker-pids");
    assert!(!pid_file.to_str().expect("test path UTF-8").contains('\''));
    // Explicit synthetic lifecycle worker, never a PATH replacement or SVM oracle.
    let source = format!("#!/usr/bin/dash\n(while :; do :; done) &\nprintf '%s %s' \"$$\" \"$!\" > '{}'\nwhile :; do :; done\n", pid_file.display());
    let executable = root.join("worker");
    fs::write(&executable, &source).expect("test worker");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).expect("worker mode");
    let binary_hash = file_sha256(&executable).expect("worker digest");
    let source_hash = Digest::of(source.as_bytes());
    let build_hash = Digest::of(b"explicit synthetic lifecycle worker build v1");
    let executor = "synthetic-cli-cancel-worker";
    let capabilities = json!(["guarded-generic-sysvars/v1"]);
    let catalog = json!({"schema":"svm-replay-workers/v1","platform":{"os":"linux","arch":"x86_64","glibcMin":"2.17"},
        "workers":[{"family":"v2-3","file":"worker","sha256":binary_hash,"buildHash":build_hash,"sourceSha256":source_hash,
            "executorSourceId":executor,"capabilities":capabilities}]});
    let catalog_hash = write_json(&root.join("catalog.json"), &catalog);
    let mut wire = vec![1];
    wire.extend([7; 64]);
    wire.extend([1, 0, 1, 2]);
    wire.extend([1; 32]);
    wire.extend([2; 32]);
    wire.extend([3; 32]);
    wire.extend([1, 1, 1, 0, 2, 4, 5]);
    let encoded = STANDARD.encode(&wire);
    let archive = transaction::decode(&encoded).expect("synthetic canonical transaction");
    let block = json!({"result":{"parentSlot":19,"transactions":[archive]}});
    let block_bytes = serde_json::to_vec(&block).expect("archived test block");
    let block_hash = Digest::of(&block_bytes);
    let mut clock_bytes = [0u8; 40];
    clock_bytes[..8].copy_from_slice(&20u64.to_le_bytes());
    let context_hash = Digest::of(b"synthetic lifecycle context");
    let mut request = json!({"schema":"svm-replay-prepared/v1","requestId":"cli-cancellation-test","family":"v2-3",
        "candidate":{"slot":20,"transactionIndex":0,"blockSourceHash":block_hash,"executorSourceId":executor},
        "fixture":{"target":{"targetSlot":20,"parentSlot":19,"index":0,"transactionBase64":encoded,
            "prefixIndices":[],"prefixTransactionsBase64":[],"replacementTransactionBase64":null},"accountOverride":null,"accounts":[],
            "clock":{"pubkey":"SysvarC1ock11111111111111111111111111111111","sourceSlot":20,"role":"sysvar","presence":"present",
                "owner":"Sysvar1111111111111111111111111111111111111","executable":false,"lamports":"1","rentEpoch":"18446744073709551615","dataBase64":STANDARD.encode(clock_bytes)},
            "runtime":{"lastRestartSlot":{"bindingHash":context_hash},
                "bankContext":{"targetSlot":20,"parentSlot":19,"executionBlockhash":block["result"]["transactions"][0]["transaction"]["message"]["recentBlockhash"],
                    "lamportsPerSignature":5,"sourceKind":"synthetic-lifecycle-test","blockSourceHash":block_hash,"blockLineageHash":context_hash,
                    "feeWitnessCount":0,"feeWitnessIndicesHash":Digest::of(b"[]"),"feeWitnessHash":Digest::of(b"[]"),"requiredRuntimeSysvars":[]},
                "binding":{"bindingHash":context_hash,"executor":{"id":executor,"sourceEvidenceHash":source_hash,
                "m9Build":{"binarySha256":binary_hash,"buildHash":build_hash,"capabilities":capabilities}}}}},
        "rawBlockBase64":STANDARD.encode(block_bytes),"blockSha256":block_hash,"sourceEvidenceHashes":[source_hash],
        "metadataPolicy":"STRICT","limits":{"timeoutMs":10000,"maxOutputBytes":4096,"maxDiagnosticBytes":4096}});
    let profile = Digest::of(b"synthetic cancel profile");
    request["candidate"]["runtimeProfileId"] = json!(profile);
    let binding = &mut request["fixture"]["runtime"]["binding"];
    binding["schema"] = json!("svm-simulate-m6-runtime-binding/v1");
    binding["targetSlot"] = json!(20);
    binding["runtimeProfileId"] = json!(profile);
    binding["features"] = json!([]);
    binding["activeFeatureSetHash"] = json!(Digest::of(b"svm-feature-set/v1\n\n"));
    binding["enabledBuiltinsHash"] = json!(Digest::of(b"synthetic builtins"));
    binding["enabledPrecompilesHash"] = json!(Digest::of(b"synthetic precompiles"));
    binding.as_object_mut().unwrap().remove("bindingHash");
    binding["bindingHash"] = json!(Digest::of(format!(
        "m6-runtime-binding/v1\n{}\n",
        serde_json::to_string(binding).unwrap()
    )));
    write_json(&root.join("request.json"), &request);
    (catalog_hash, pid_file)
}

struct CliChild(Child);
impl Drop for CliChild {
    fn drop(&mut self) {
        match self.0.try_wait() {
            Ok(Some(_)) => {}
            state => {
                if let Err(error) = state {
                    eprintln!("test child state: {error}");
                }
                if let Err(error) = self.0.kill() {
                    eprintln!("test final child kill: {error}");
                }
                if let Err(error) = self.0.wait() {
                    eprintln!("test final child wait: {error}");
                }
            }
        }
    }
}

#[test]
fn sigint_and_sigterm_publish_cancelled_receipts_after_descendants_are_reaped() {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let root = tempfile::tempdir().expect("test root");
        let (catalog_hash, pid_file) = prepare(root.path());
        let mut child = CliChild(
            Command::new(env!("CARGO_BIN_EXE_svm-replay"))
                .arg("simulate")
                .arg("--request")
                .arg(root.path().join("request.json"))
                .arg("--catalog")
                .arg(root.path().join("catalog.json"))
                .arg("--catalog-sha256")
                .arg(catalog_hash)
                .arg("--data-dir")
                .arg(root.path().join("data"))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("CLI spawn"),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !pid_file.exists() {
            if let Some(status) = child.0.try_wait().expect("poll CLI") {
                let mut diagnostic = String::new();
                use std::io::Read;
                child
                    .0
                    .stdout
                    .as_mut()
                    .expect("CLI stdout")
                    .read_to_string(&mut diagnostic)
                    .expect("read early error");
                panic!("CLI exited before worker started: {status}: {diagnostic}");
            }
            assert!(Instant::now() < deadline, "worker did not start");
            thread::sleep(Duration::from_millis(5));
        }
        let pids: Vec<i32> = fs::read_to_string(&pid_file)
            .expect("worker PID evidence")
            .split_whitespace()
            .map(|text| text.parse().expect("numeric worker PID"))
            .collect();
        assert_eq!(pids.len(), 2);
        assert_eq!(
            unsafe { libc::kill(child.0.id() as i32, signal) },
            0,
            "send actual CLI signal"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.0.try_wait().expect("poll cancelled CLI") {
                break status;
            }
            assert!(Instant::now() < deadline, "cancelled CLI did not exit");
            thread::sleep(Duration::from_millis(5));
        };
        use std::io::Read;
        let mut stdout = Vec::new();
        child
            .0
            .stdout
            .as_mut()
            .expect("CLI stdout")
            .read_to_end(&mut stdout)
            .expect("read CLI receipt");
        assert!(!status.success());
        let receipt: Value =
            serde_json::from_slice(&stdout).expect("cooperative signal produces JSON receipt");
        assert_eq!(receipt["outcome"], "CANCELLED", "{receipt}");
        assert_eq!(receipt["error"]["code"], "WORKER_CANCELLED");
        let stored: Value = serde_json::from_slice(
            &fs::read(
                receipt["receiptPath"]
                    .as_str()
                    .expect("durable receipt path"),
            )
            .expect("durable receipt"),
        )
        .expect("receipt JSON");
        assert_eq!(stored["outcome"], "CANCELLED");
        for pid in pids {
            assert_eq!(
                unsafe { libc::kill(pid, 0) },
                -1,
                "PID {pid} must be reaped before receipt"
            );
        }
    }
}
