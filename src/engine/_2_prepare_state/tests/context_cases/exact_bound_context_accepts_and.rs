use super::*;

#[test]
fn exact_bound_context_accepts_and_preserves_inputs() {
    let value = fixture();
    let before = value.clone();
    assert_bound_context(&value).unwrap();
    assert_epoch_stake_binding(&value, Some(&key(1))).unwrap();
    assert_initialized_stake_binding(&value, Some(&key(1))).unwrap();
    assert_eq!(value, before);
}

#[test]
fn prepared_hydration_rejects_mixed_genesis_before_creating_a_job_or_reading_sources() {
    use svm_replay_engine::{
        shared::{
            runtime::{CancellationToken, ProcessOwner},
            sources::{CompositeSource, HistoricalSource, SourceError},
        },
        {simulate_prepared_with_sources, CacheMode, Config},
    };
    use svm_replay_protocol::{Digest, Limits, MetadataPolicy, PreparedRequest};
    struct NoReads(String);
    impl HistoricalSource for NoReads {
        fn identity(&self) -> Value {
            json!({"id":"genesis-admission-test","version":"1","kind":"captured-history",
                "genesisHash":self.0,"coverage":{"firstSlot":0,"lastSlot":20,"completeness":"partial"},"capabilities":["account"]})
        }
        fn inspect(&self, _: &Value) -> Result<Option<Value>, SourceError> {
            panic!("genesis mismatch must fail before source acquisition")
        }
    }
    for removed in [None, Some("epochStakes"), Some("initializedStakeSnapshot")] {
        let root = tempfile::tempdir().unwrap();
        let mut fixture = fixture();
        if let Some(name) = removed {
            fixture["runtime"].as_object_mut().unwrap().remove(name);
        }
        assert_bound_context(&fixture).unwrap();
        let raw = br#"{"result":{"parentSlot":19,"transactions":[]}}"#;
        let block_sha256 = Digest::of(raw);
        let request = PreparedRequest {
            schema: "svm-replay-prepared/v1".into(),
            request_id: "genesis-admission".into(),
            family: "v4-2".into(),
            candidate: json!({"slot":20,"blockSourceHash":block_sha256}),
            fixture,
            raw_block_base64: STANDARD.encode(raw),
            block_sha256,
            source_evidence_hashes: vec![Digest::of(b"synthetic evidence")],
            metadata_policy: MetadataPolicy::Strict,
            limits: Limits {
                timeout_ms: 1000,
                max_output_bytes: 1024,
                max_diagnostic_bytes: 1024,
            },
        };
        request.validate().unwrap();
        let config = Config {
            catalog_path: root.path().join("nonexistent-catalog"),
            catalog_sha256: Digest::of(b"no catalog"),
            data_dir: root.path().join("must-not-create"),
            owner: ProcessOwner {
                executable: root.path().join("nonexistent-owner"),
                sha256: hash(b"no owner"),
            },
            cache: CacheMode::Off,
            trace: None,
        };
        let wrong_genesis = key(2);
        let sources = CompositeSource::new(vec![Box::new(NoReads(wrong_genesis.clone()))]).unwrap();
        let error = simulate_prepared_with_sources(
            request,
            &config,
            sources,
            wrong_genesis,
            CancellationToken::new(),
        )
        .expect_err("bound Bank inputs and requested source genesis must agree");
        assert_eq!(
            error.code,
            if removed == Some("epochStakes") {
                "INVALID_INITIALIZED_STAKE_INPUT"
            } else {
                "INVALID_EPOCH_STAKE_INPUT"
            }
        );
        assert!(error.message.contains("genesis"));
        assert!(
            !config.data_dir.exists(),
            "no job/scratch/catalog/worker work before genesis admission"
        );
    }
}

#[test]
fn source_evidence_binders_compute_proofs_and_replace_only_exact_parent_images() {
    let fixture = fixture();
    let input = json!({"fixture":fixture,"evidence":evidence(&fixture["runtime"]["epochStakes"])});
    assert_eq!(
        bind_epoch_stake_evidence(&input).unwrap(),
        fixture["runtime"]["epochStakes"]
    );
    let mut input = initialized_request(&fixture);
    let unrelated =
        json!({"pubkey":key(12),"sourceSlot":19,"role":"application","presence":"absent"});
    input["accounts"]
        .as_array_mut()
        .unwrap()
        .push(unrelated.clone());
    let before = input.clone();
    let result = prepare_exact_initialized_stakes(&input).unwrap();
    assert_eq!(input, before);
    assert_eq!(
        result["snapshot"],
        fixture["runtime"]["initializedStakeSnapshot"]
    );
    assert_eq!(
        result["accounts"],
        json!([fixture["accounts"][0], unrelated])
    );
    assert_eq!(prepare_exact_initialized_stakes(&input).unwrap(), result);
    let mut already_initialized = input.clone();
    already_initialized["accounts"] = result["accounts"].clone();
    assert!(prepare_exact_initialized_stakes(&already_initialized).is_err());
}
