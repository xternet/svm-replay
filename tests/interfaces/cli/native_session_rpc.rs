use crate::{unix::inputs, verified::reviewed_input};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    time::Duration,
};

pub(super) struct Owned(pub(super) Child);
impl Drop for Owned {
    fn drop(&mut self) {
        match self.0.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {
                if let Err(error) = self.0.kill() {
                    eprintln!("test child kill failed: {error}");
                }
            }
            Err(error) => eprintln!("test child status failed: {error}"),
        }
        if let Err(error) = self.0.wait() {
            eprintln!("test child wait failed: {error}");
        }
    }
}

fn post(address: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    write!(stream, "POST / HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).unwrap();
    stream.write_all(body).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let end = bytes
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .unwrap()
        + 4;
    let header = std::str::from_utf8(&bytes[..end]).unwrap();
    let status = header
        .split_ascii_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    (status, bytes[end..].to_vec())
}

pub(super) fn rpc(address: &str, method: &str, params: Value) -> Value {
    let (status, body) = post(
        address,
        &serde_json::to_vec(&json!({"jsonrpc":"2.0","id":7,"method":method,"params":params}))
            .unwrap(),
    );
    assert_eq!(status, 200);
    serde_json::from_slice(&body).unwrap()
}

#[test]
fn occupied_session_port_refuses_before_loading_input_or_creating_scratch() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .args(["session", "--request", "missing.json", "--port"])
        .arg(listener.local_addr().unwrap().port().to_string())
        .arg("--data-dir")
        .arg(root.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["error"]["code"], "SESSION_BIND");
    assert!(!root.path().join("scratch").exists());
}

#[test]
#[ignore = "requires reviewed SVM_SESSION_RECEIPT/WORKER pins; explicit cached integration driver"]
fn ordinary_native_cli_serves_verified_rpc_and_closes_its_worker() {
    let (_, worker, _) = inputs();
    let input = reviewed_input(worker.executable);
    let receipt: Value =
        serde_json::from_slice(&std::fs::read(&input.receipt_path).unwrap()).unwrap();
    let result: Value = serde_json::from_slice(
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
    let row = result["original"]["accountTransitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["after"]["presence"] == "present")
        .unwrap();
    let account = row["pubkey"].as_str().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("session.json");
    std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let mut child = Owned(
        Command::new(env!("CARGO_BIN_EXE_svm-replay"))
            .args(["session", "--request"])
            .arg(&path)
            .arg("--data-dir")
            .arg(root.path())
            .args(["--timeout", "120"])
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
    let slot = &ready["pins"]["slot"];
    assert_eq!(rpc(address, "getHealth", json!([]))["result"], "ok");
    assert_eq!(rpc(address, "getSlot", json!([]))["result"], *slot);
    assert_eq!(
        rpc(address, "getBlockHeight", json!([]))["result"],
        ready["pins"]["blockHeight"]
    );
    assert_eq!(
        rpc(address, "getVersion", json!([]))["result"]["solana-core"],
        receipt["runtime"]["agave"]
    );
    assert_eq!(
        rpc(address, "getGenesisHash", json!([]))["result"],
        ready["pins"]["genesisHash"]
    );
    let image = rpc(
        address,
        "getAccountInfo",
        json!([account,{"encoding":"base64","minContextSlot":slot}]),
    )["result"]["value"]
        .clone();
    assert_eq!(
        image["lamports"].as_u64().unwrap().to_string(),
        row["after"]["lamports"]
    );
    assert_eq!(image["owner"], row["after"]["owner"]);
    let bytes = STANDARD.decode(image["data"][0].as_str().unwrap()).unwrap();
    assert_eq!(
        svm_replay_protocol::Digest::of(&bytes).as_str(),
        row["after"]["dataHash"]
    );
    assert_eq!(
        rpc(address, "getBalance", json!([account]))["result"]["value"],
        image["lamports"]
    );
    assert_eq!(
        rpc(
            address,
            "getMultipleAccounts",
            json!([[account],{"encoding":"base64"}])
        )["result"]["value"][0],
        image
    );
    let unknown = rpc(
        address,
        "getAccountInfo",
        json!(["Vote111111111111111111111111111111111111111",{"encoding":"base64"}]),
    );
    assert_eq!(unknown["error"]["data"]["code"], "NEEDS_INPUT");
    let initial = rpc(address, "svmReplay_getState", json!([]))["result"].clone();
    let branch = rpc(address, "svmReplay_snapshot", json!([]))["result"]["forkId"].clone();
    let selected = rpc(address, "svmReplay_selectFork", json!([branch]))["result"].clone();
    assert_eq!(selected["stateSha256"], initial["stateSha256"]);
    rpc(address, "svmReplay_reset", json!([]));
    rpc(address, "svmReplay_selectFork", json!([initial["forkId"]]));
    assert_eq!(
        rpc(address, "svmReplay_closeFork", json!([branch]))["result"]["closedFork"],
        branch
    );
    let batch = json!([{"jsonrpc":"2.0","id":"a","method":"getSlot"},
        {"jsonrpc":"2.0","id":"b","method":"unknown"},{"jsonrpc":"2.0","method":"getSlot"}]);
    let (status, body) = post(address, &serde_json::to_vec(&batch).unwrap());
    assert_eq!(status, 200);
    let batch: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(batch.as_array().unwrap().len(), 2);
    assert_eq!(batch[0]["result"], *slot);
    assert_eq!(batch[1]["error"]["code"], -32601);
    let (status, body) = post(address, br#"{"jsonrpc":"2.0","method":"getSlot"}"#);
    assert_eq!(status, 204);
    assert!(body.is_empty());
    let (status, body) = post(address, b"{");
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["error"]["code"],
        -32700
    );
    assert_eq!(
        rpc(address, "svmReplay_getState", json!([]))["result"],
        initial
    );
    assert_eq!(
        rpc(address, "svmReplay_close", json!([]))["result"]["closed"],
        true
    );
    let mut final_output = String::new();
    output.read_to_string(&mut final_output).unwrap();
    assert!(child.0.wait().unwrap().success(), "{final_output}");
    let closed: Value = serde_json::from_str(&final_output).unwrap();
    assert_eq!(closed["outcome"], "COMPLETED");
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
