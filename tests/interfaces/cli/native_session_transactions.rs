use crate::{
    rpc::{rpc, Owned},
    unix::inputs,
    verified::reviewed_input,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read},
    process::{Command, Stdio},
};

#[test]
#[ignore = "requires reviewed SVM_SESSION_RECEIPT/WORKER pins; explicit cached integration driver"]
fn ordinary_rpc_replays_signed_history_and_preserves_simulation_duplicate_reset_semantics() {
    let (fixture, worker, _) = inputs();
    let mut input = reviewed_input(worker.executable);
    input.after = false;
    let receipt: Value =
        serde_json::from_slice(&std::fs::read(&input.receipt_path).unwrap()).unwrap();
    let expected: Value = serde_json::from_slice(
        &std::fs::read(
            input
                .receipt_path
                .parent()
                .unwrap()
                .join(receipt["output"]["file"].as_str().unwrap()),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        expected["original"]["status"], "ok",
        "this integration check needs a successful original"
    );
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("session.json");
    std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let mut child = Owned(
        Command::new(env!("CARGO_BIN_EXE_svm-replay"))
            .args(["session", "--request"])
            .arg(path)
            .arg("--data-dir")
            .arg(root.path())
            .args(["--timeout", "180"])
            .env_remove("API_ALCHEMY")
            .env_remove("API_ALCHEMY_SOLANA_HIST")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let mut output = BufReader::new(child.0.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let ready: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(ready["kind"], "ready", "{ready}");
    let address = ready["url"]
        .as_str()
        .unwrap()
        .strip_prefix("http://")
        .unwrap();
    let initial = rpc(address, "svmReplay_getState", json!([]))["result"].clone();
    let signature = fixture["target"]["signature"].clone();
    let wire = fixture["target"]["transactionBase64"].as_str().unwrap();
    let latest = rpc(address, "getLatestBlockhash", json!([]));
    assert_eq!(
        latest["result"]["value"]["blockhash"],
        ready["pins"]["executionBlockhash"]
    );
    assert_ne!(
        latest["result"]["value"]["blockhash"],
        ready["pins"]["blockhash"]
    );
    assert!(
        rpc(address, "getSignatureStatuses", json!([[signature]]))["result"]["value"][0].is_null()
    );
    let simulated = rpc(
        address,
        "simulateTransaction",
        json!([wire,{"encoding":"base64","replaceRecentBlockhash":true}]),
    );
    assert!(simulated.get("error").is_none(), "{simulated}");
    assert_eq!(simulated["result"]["value"]["err"], Value::Null);
    assert_eq!(
        simulated["result"]["value"]["unitsConsumed"],
        expected["original"]["computeUnits"]
    );
    assert_eq!(
        simulated["result"]["value"]["logs"],
        expected["original"]["logs"]
    );
    assert_eq!(
        rpc(address, "svmReplay_getState", json!([]))["result"],
        initial
    );
    let mut invalid = STANDARD.decode(wire).unwrap();
    invalid[1] ^= 1;
    let rejected = rpc(
        address,
        "sendTransaction",
        json!([STANDARD.encode(invalid),{"encoding":"base64","skipPreflight":true}]),
    );
    assert_eq!(rejected["error"]["code"], -32003, "{rejected}");
    assert_eq!(
        rpc(address, "svmReplay_getState", json!([]))["result"],
        initial
    );
    let sent = rpc(
        address,
        "sendTransaction",
        json!([wire,{"encoding":"base64","skipPreflight":true}]),
    );
    assert_eq!(sent["result"], signature, "{sent}");
    let changed = rpc(address, "svmReplay_getState", json!([]))["result"].clone();
    assert_eq!(changed["slot"], initial["slot"]);
    assert_eq!(changed["transactionsCommitted"], 1);
    assert_eq!(
        changed["transactionIndex"].as_u64().unwrap(),
        initial["transactionIndex"].as_u64().unwrap() + 1
    );
    for row in expected["original"]["accountTransitions"]
        .as_array()
        .unwrap()
    {
        let response = rpc(
            address,
            "getAccountInfo",
            json!([row["pubkey"],{"encoding":"base64"}]),
        );
        assert!(response.get("error").is_none(), "{response}");
        let account = &response["result"]["value"];
        if row["after"]["presence"] == "absent" {
            assert!(account.is_null());
            continue;
        }
        assert_eq!(
            account["lamports"].as_u64().unwrap().to_string(),
            row["after"]["lamports"]
        );
        assert_eq!(account["owner"], row["after"]["owner"]);
        let bytes = STANDARD
            .decode(account["data"][0].as_str().unwrap())
            .unwrap();
        assert_eq!(
            svm_replay_protocol::Digest::of(&bytes).as_str(),
            row["after"]["dataHash"]
        );
    }
    let status = rpc(
        address,
        "getSignatureStatuses",
        json!([[signature],{"searchTransactionHistory":true}]),
    );
    assert_eq!(
        status["result"]["value"][0]["confirmationStatus"],
        "finalized"
    );
    assert!(status["result"]["value"][0]["confirmations"].is_null());
    assert_eq!(status["result"]["value"][0]["slot"], changed["slot"]);
    assert!(status["result"]["value"][0]["err"].is_null());
    let duplicate = rpc(
        address,
        "sendTransaction",
        json!([wire,{"encoding":"base64","skipPreflight":true}]),
    );
    assert_eq!(
        duplicate["error"]["data"]["err"], "AlreadyProcessed",
        "{duplicate}"
    );
    assert_eq!(
        rpc(address, "svmReplay_getState", json!([]))["result"],
        changed
    );
    let raw = STANDARD.decode(wire).unwrap();
    assert_eq!(
        raw[0], 1,
        "reviewed integration transaction has one signature"
    );
    let fee = rpc(
        address,
        "getFeeForMessage",
        json!([STANDARD.encode(&raw[65..])]),
    );
    assert_eq!(
        fee["result"]["value"].as_u64().unwrap().to_string(),
        expected["original"]["fee"],
        "{fee}"
    );
    assert_eq!(
        rpc(address, "svmReplay_reset", json!([]))["result"]["stateSha256"],
        initial["stateSha256"]
    );
    assert!(
        rpc(address, "getSignatureStatuses", json!([[signature]]))["result"]["value"][0].is_null()
    );
    assert_eq!(
        rpc(
            address,
            "sendTransaction",
            json!([wire,{"encoding":"base64","skipPreflight":true}])
        )["result"],
        signature
    );
    assert_eq!(
        rpc(address, "svmReplay_getState", json!([]))["result"]["stateSha256"],
        changed["stateSha256"]
    );
    assert_eq!(
        rpc(address, "svmReplay_close", json!([]))["result"]["closed"],
        true
    );
    let mut final_output = String::new();
    output.read_to_string(&mut final_output).unwrap();
    assert!(child.0.wait().unwrap().success(), "{final_output}");
    let closed: Value = serde_json::from_str(&final_output).unwrap();
    let cleanup = if cfg!(target_os = "macos") {
        "OwnerTerminatedProcessGroup"
    } else {
        "OwnerReapedProcessTree"
    };
    assert_eq!(closed["cleanup"], cleanup);
    assert_eq!(
        std::fs::read_dir(root.path().join("scratch"))
            .unwrap()
            .count(),
        0
    );
}
