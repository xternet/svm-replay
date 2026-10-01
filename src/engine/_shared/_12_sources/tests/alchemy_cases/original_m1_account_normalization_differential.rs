use super::*;

#[test]
#[ignore = "explicit development differential requires Bun and SVM_REPLAY_PROTOTYPE"]
fn original_m1_account_normalization_differential_preserves_exact_u64_bytes_and_absence() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut cases = vec![];
    let mut actual = vec![];
    for lamports in [0, 1, 9007199254740993, u64::MAX] {
        for rent in [0, 42, u64::MAX] {
            for as_string in [false, true] {
                for absent in [false, true] {
                    let mut response = account();
                    let value = &mut response[0]["result"]["value"];
                    if absent {
                        *value = Value::Null;
                    } else {
                        value["lamports"] = if as_string {
                            json!(lamports.to_string())
                        } else {
                            json!(lamports)
                        };
                        value["rentEpoch"] = if as_string {
                            json!(rent.to_string())
                        } else {
                            json!(rent)
                        };
                    }
                    let raw = serde_json::to_vec(&response).unwrap();
                    let source = AlchemySource::with_transport(
                        config(),
                        limits(),
                        Transport::values(vec![genesis(), response]),
                    )
                    .unwrap();
                    actual.push(source.inspect(&query(2)).unwrap().unwrap()["value"].clone());
                    cases
                        .push(json!({"bodyBase64":STANDARD.encode(raw),"pubkey":key(2),"slot":99}));
                }
            }
        }
    }
    let script = r#"const root=process.env.SVM_REPLAY_PROTOTYPE;const {M1RpcClient}=await import(root+'/poc/m1-small-defi/rpc.ts');const {buildStrictHistoricalAccounts}=await import(root+'/poc/m0-correctness/fixture.ts');const results=[];for(const c of await Bun.stdin.json()){const body=Buffer.from(c.bodyBase64,'base64').toString();const client=new M1RpcClient('https://synthetic.invalid/public',async()=>new Response(body));const {account}=await client.exactHistoricalAccount(c.pubkey,c.slot);results.push(buildStrictHistoricalAccounts([account],c.slot,{programs:new Set(),programData:new Set(),addressLookupTables:new Set(),sysvars:new Set(),excluded:new Set()})[0]);}console.log(JSON.stringify(results));"#;
    let mut child = Command::new("bun")
        .args(["--eval", script])
        .env(
            "SVM_REPLAY_PROTOTYPE",
            std::env::var("SVM_REPLAY_PROTOTYPE").expect("prototype path"),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&cases).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = svm_replay_protocol::parse_json(&output.stdout).unwrap();
    assert_eq!(Value::Array(actual), expected);
}
