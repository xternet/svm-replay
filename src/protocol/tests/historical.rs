use serde_json::json;
use svm_replay_protocol::{Digest, HistoricalRequest};
fn request() -> serde_json::Value {
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
    value
}
#[test]
fn explicit_source_request_rejects_runtime_and_phase_ambiguity() {
    let original = request();
    assert!(HistoricalRequest::parse(&serde_json::to_vec(&original).unwrap()).is_ok());
    for path in [
        "/runtimeBinding/targetSlot",
        "/runtimeBinding/executor/id",
        "/runtimeBinding/features/0/activationSlot",
        "/genesisHash",
        "/candidate/blockSourceHash",
    ] {
        let mut invalid = original.clone();
        *invalid.pointer_mut(path).unwrap() = json!(99);
        assert!(
            HistoricalRequest::parse(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "{path}"
        );
    }
    for names in [
        json!(["unknown"]),
        json!(["epochStakeEvidence", "epochStakeEvidence"]),
        json!(["initializedStakeEvidence", "initializationEvidence"]),
    ] {
        let mut invalid = original.clone();
        invalid["bankInputs"] = names;
        assert!(HistoricalRequest::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    let mut invalid = original;
    invalid["unreviewedDefault"] = json!(true);
    assert!(HistoricalRequest::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
}

#[test]
fn historical_22_family_still_requires_exact_binding() {
    let mut value = request();
    value["family"] = json!("v2-2");
    assert!(HistoricalRequest::parse(&serde_json::to_vec(&value).unwrap()).is_ok());
    value["runtimeBinding"]["executor"]["id"] = json!("wrong-runtime");
    assert!(HistoricalRequest::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    value = request();
    value["family"] = json!("v2-1");
    assert_eq!(
        HistoricalRequest::parse(&serde_json::to_vec(&value).unwrap())
            .unwrap_err()
            .code,
        "UNSUPPORTED_RUNTIME"
    );
}
