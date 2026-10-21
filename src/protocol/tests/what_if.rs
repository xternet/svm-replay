//! What-if requests: a bundle after or instead of the target, on eth_call-style state overrides.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use svm_replay_protocol::{Digest, HistoricalRequest};

/// A minimal legacy transaction: one signer paying, one program, one instruction.
fn wire(seed: u8) -> String {
    let mut bytes = vec![1];
    bytes.extend_from_slice(&[seed; 64]);
    bytes.extend_from_slice(&[1, 0, 1, 2]);
    bytes.extend_from_slice(&[7; 32]);
    bytes.extend_from_slice(&[8; 32]);
    bytes.extend_from_slice(&[9; 32]);
    bytes.extend_from_slice(&[1, 1, 1, 0, 0]);
    STANDARD.encode(bytes)
}

fn request(what_if: Value) -> Value {
    let feature = "11111111111111111111111111111111";
    let profile = Digest::of(b"synthetic historical profile");
    let mut value = json!({"schema":"svm-replay-historical/v1","requestId":"test","family":"v4-2",
        "genesisHash":"11111111111111111111111111111111","candidate":{"slot":20,"transactionIndex":0,
            "blockSourceHash":"a".repeat(64),"runtimeProfileId":profile,"executorSourceId":"executor","litesvmCommit":"commit","activeFeatures":[feature]},
        "runtimeBinding":{"schema":"svm-simulate-m6-runtime-binding/v1","targetSlot":20,"runtimeProfileId":profile,"executor":{"id":"executor","litesvmCommit":"commit"},
            "features":[{"id":feature,"activationSlot":19}],"activeFeatureSetHash":Digest::of(format!("svm-feature-set/v1\n{feature}\n")),
            "enabledBuiltinsHash":Digest::of(b"builtins"),"enabledPrecompilesHash":Digest::of(b"precompiles")},"replacementTransactionBase64":null,"requestedAccountOverrides":null,
        "bankInputs":[],"metadataPolicy":"STRICT","limits":{"timeoutMs":1000,"maxOutputBytes":1024,"maxDiagnosticBytes":1024}});
    value["runtimeBinding"]["bindingHash"] = json!(Digest::of(format!(
        "m6-runtime-binding/v1\n{}\n",
        serde_json::to_string(&value["runtimeBinding"]).unwrap()
    )));
    if !what_if.is_null() {
        value["whatIf"] = what_if;
    }
    value
}

fn parse(value: &Value) -> Result<HistoricalRequest, svm_replay_protocol::Error> {
    HistoricalRequest::parse(&serde_json::to_vec(value).unwrap())
}

const ACCOUNT: &str = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";

#[test]
fn bundles_after_or_instead_of_the_target_parse() {
    for mode in ["after", "replace"] {
        let value = request(json!({"mode":mode,"transactionsBase64":[wire(1), wire(2)]}));
        let parsed = parse(&value).unwrap();
        assert_eq!(
            serde_json::to_value(&parsed).unwrap()["whatIf"],
            value["whatIf"]
        );
    }
    let rerun =
        request(json!({"mode":"replace","stateOverrides":[{"pubkey":ACCOUNT,"lamports":"5"}]}));
    assert!(parse(&rerun).is_ok(), "overrides alone re-run the target");
}

#[test]
fn requests_without_what_if_keep_their_bytes() {
    let parsed = parse(&request(Value::Null)).unwrap();
    assert!(serde_json::to_value(&parsed)
        .unwrap()
        .get("whatIf")
        .is_none());
}

#[test]
fn malformed_bundles_are_refused() {
    let too_many: Vec<_> = (0..17).map(wire).collect();
    for what_if in [
        json!({"mode":"after"}),
        json!({"mode":"replace"}),
        json!({"mode":"after","transactionsBase64":[]}),
        json!({"mode":"after","transactionsBase64":too_many}),
        json!({"mode":"after","transactionsBase64":["not-a-transaction"]}),
        json!({"mode":"sideways","transactionsBase64":[wire(1)]}),
        json!({"mode":"after","transactionsBase64":[wire(1)],"extra":true}),
    ] {
        assert!(parse(&request(what_if.clone())).is_err(), "{what_if}");
    }
    let mut legacy = request(json!({"mode":"after","transactionsBase64":[wire(1)]}));
    legacy["replacementTransactionBase64"] = json!(wire(3));
    assert!(
        parse(&legacy).is_err(),
        "whatIf excludes the legacy replacement"
    );
    let mut legacy = request(json!({"mode":"after","transactionsBase64":[wire(1)]}));
    legacy["requestedAccountOverrides"] = json!([{"pubkey":ACCOUNT,"lamports":"1"}]);
    assert!(parse(&legacy).is_err(), "whatIf excludes legacy overrides");
}

#[test]
fn state_overrides_follow_eth_call_rules() {
    let ok = |entry: Value| {
        parse(&request(
            json!({"mode":"after","transactionsBase64":[wire(1)],"stateOverrides":[entry]}),
        ))
    };
    for entry in [
        json!({"pubkey":ACCOUNT,"lamports":"0"}),
        json!({"pubkey":ACCOUNT,"owner":"BPFLoader2111111111111111111111111111111111","executable":true,"dataBase64":"f0VMRg=="}),
        json!({"pubkey":ACCOUNT,"rentEpoch":"18446744073709551615","dataBase64":""}),
    ] {
        assert!(ok(entry.clone()).is_ok(), "{entry}");
    }
    for entry in [
        json!({"pubkey":ACCOUNT}),
        json!({"pubkey":"short","lamports":"1"}),
        json!({"pubkey":ACCOUNT,"lamports":"01"}),
        json!({"pubkey":ACCOUNT,"lamports":"18446744073709551616"}),
        json!({"pubkey":ACCOUNT,"lamports":5}),
        json!({"pubkey":ACCOUNT,"rentEpoch":"-1"}),
        json!({"pubkey":ACCOUNT,"owner":"not base58!"}),
        json!({"pubkey":ACCOUNT,"dataBase64":"f0VMRg"}),
        json!({"pubkey":ACCOUNT,"program":"@x.so"}),
    ] {
        assert!(ok(entry.clone()).is_err(), "{entry}");
    }
    let twice = json!({"pubkey":ACCOUNT,"lamports":"1"});
    assert!(parse(&request(
        json!({"mode":"after","transactionsBase64":[wire(1)],"stateOverrides":[twice, twice]})
    ))
    .is_err());
    let many: Vec<_> = (0..65_u8)
        .map(|n| json!({"pubkey":bs58::encode([n + 1; 32]).into_string(),"lamports":"1"}))
        .collect();
    assert!(parse(&request(
        json!({"mode":"after","transactionsBase64":[wire(1)],"stateOverrides":many})
    ))
    .is_err());
}
