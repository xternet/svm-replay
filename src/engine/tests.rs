use crate::_5_finalize::attempts::archive_failed_attempt;
use crate::_5_finalize::sources::record_sources;
use crate::shared::{
    history::History,
    runtime::{CancellationToken, ExecutionBudget},
    sources::CompositeSource,
};
use crate::*;
use serde_json::{json, Value};
use std::time::Duration;
use svm_replay_protocol::Digest;

#[test]
fn missing_archive_data_names_the_provider_without_changing_the_error_code() {
    use crate::shared::sources::{HistoricalSource, SourceError};
    struct Source;
    impl HistoricalSource for Source {
        fn identity(&self) -> Value {
            json!({"id":"archive-test","version":"1","kind":"alchemy-historical","genesisHash":bs58::encode([1;32]).into_string(),
                "coverage":{"firstSlot":0,"lastSlot":20,"completeness":"partial"},"capabilities":["account"]})
        }
        fn inspect(&self, _: &Value) -> Result<Option<Value>, SourceError> {
            Ok(None)
        }
    }
    let job = tempfile::tempdir().unwrap();
    let mut sources = CompositeSource::new(vec![Box::new(Source)]).unwrap();
    let key = bs58::encode([2; 32]).into_string();
    sources.inspect(&json!({"kind":"account","genesisHash":bs58::encode([1;32]).into_string(),"pubkey":key,"slot":19,"phase":"end-slot"})).unwrap();
    let budget = ExecutionBudget::new(Duration::from_secs(1), CancellationToken::new()).unwrap();
    let history = History::new(
        &mut sources,
        bs58::encode([1; 32]).into_string(),
        20,
        &budget,
        1,
    )
    .unwrap();
    let mut receipt = json!({});
    let error = record_sources(
        Err(Error::new(
            "UNSUPPORTED_HISTORICAL_ACCOUNT",
            "archive returned no account image",
        )
        .with_details(json!({"pubkey":key,"parentSlot":19}))),
        &history,
        job.path(),
        &mut receipt,
    )
    .unwrap_err();
    assert_eq!(error.code, "UNSUPPORTED_HISTORICAL_ACCOUNT");
    assert!(error
        .message
        .contains("Missing historical data from provider: Alchemy"));
    let details = error.details.unwrap();
    assert_eq!(details["pubkey"], key);
    assert_eq!(
        details["providerFailure"]["kind"],
        "PROVIDER_DATA_UNAVAILABLE"
    );
    let unrelated = record_sources(
        Err(Error::new(
            "UNSUPPORTED_HISTORICAL_ACCOUNT",
            "not observed from that provider",
        )
        .with_details(json!({"pubkey":bs58::encode([3;32]).into_string(),"parentSlot":19}))),
        &history,
        job.path(),
        &mut receipt,
    )
    .unwrap_err();
    assert!(!unrelated.message.contains("provider:"));
    assert!(unrelated.details.unwrap().get("providerFailure").is_none());
}

#[test]
fn source_diagnostics_are_persisted_on_failed_jobs_without_overwriting_failure() {
    use crate::shared::sources::{HistoricalSource, SourceError};
    struct Source;
    impl HistoricalSource for Source {
        fn identity(&self) -> Value {
            json!({"id":"test-local","version":"1","kind":"captured-history","genesisHash":bs58::encode([1;32]).into_string(),
                "coverage":{"firstSlot":0,"lastSlot":20,"completeness":"partial"},"capabilities":["account"]})
        }
        fn inspect(&self, _: &Value) -> Result<Option<Value>, SourceError> {
            panic!("diagnostics must not acquire inputs")
        }
    }
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let copy = events.clone();
    let progress = crate::shared::progress::ProgressJob::start(
        crate::shared::progress::Phase::FetchingHistoricalAccounts,
        std::sync::Arc::new(move |event| copy.lock().unwrap().push(event)),
    );
    let job = tempfile::tempdir().unwrap();
    let mut sources = CompositeSource::new(vec![Box::new(Source)]).unwrap();
    let budget = ExecutionBudget::new(Duration::from_secs(1), CancellationToken::new()).unwrap();
    let history = History::new(
        &mut sources,
        bs58::encode([1; 32]).into_string(),
        20,
        &budget,
        1,
    )
    .unwrap();
    let mut receipt = json!({"controlVerification":null,"verification":null});
    let result = record_sources(
        Err(Error::new("SOURCE_TRANSPORT", "earlier acquisition failed")),
        &history,
        job.path(),
        &mut receipt,
    );
    let phase = events.lock().unwrap().last().unwrap().phase;
    assert_eq!(phase, crate::shared::progress::Phase::SavingInputs);
    let receipt = finish_job(result, receipt, job.path(), Instant::now()).unwrap();
    progress.finish("ERROR");
    assert_eq!(receipt["error"]["code"], "SOURCE_TRANSPORT");
    assert_eq!(
        receipt["sourceDiagnostics"]["sources"][0]["diagnostics"]["status"],
        "NOT_REPORTED"
    );
    let saved: Value =
        serde_json::from_slice(&std::fs::read(job.path().join("receipt.json")).unwrap()).unwrap();
    assert_eq!(saved["sourceDiagnostics"], receipt["sourceDiagnostics"]);
}

#[test]
fn discarded_discovery_retains_only_archived_control_evidence() {
    let job = tempfile::tempdir().unwrap();
    // Protocol lifecycle test; this verification is not historical evidence.
    let verification = json!({"status":"PASS","evidenceHash":Digest::of(b"protocol-only")});
    let proof = _4_simulate::control::evidence(
        &json!({"attempt":0}),
        &json!({"observed":true}),
        &verification,
    );
    let mut receipt = json!({"phases":[],"controlVerification":verification,"controlEvidence":proof,"verification":null});
    let mut error = _4_simulate::control::attach(
        Error::new("NEEDS_INPUT", "requested missing input")
            .with_details(json!({"schema":"svm-sysvar-discovery/v1"})),
        Some(&proof),
    );
    archive_failed_attempt(job.path(), &mut receipt, 0, &mut error).unwrap();
    assert!(receipt["controlVerification"].is_null());
    assert!(receipt.get("controlEvidence").is_none());
    assert!(error
        .details
        .as_ref()
        .unwrap()
        .get("controlEvidence")
        .is_none());
    let archived: Value =
        serde_json::from_slice(&std::fs::read(job.path().join("attempt-0.json")).unwrap()).unwrap();
    assert_eq!(archived["controlEvidence"], proof);
    assert_eq!(archived["disposition"], "DISCARDED_NEEDS_INPUT");
    assert_eq!(archived["partialResultPublished"], false);
    assert_eq!(
        receipt["failedAttempts"][0]["sha256"],
        json!(Digest::of(
            std::fs::read(job.path().join("attempt-0.json")).unwrap()
        ))
    );
    let final_error = Error::new("WORKER_TIMEOUT", "next attempt failed before control");
    let finalized = finish_job(Err(final_error), receipt, job.path(), Instant::now()).unwrap();
    assert!(finalized["controlVerification"].is_null());
    assert!(finalized.get("output").is_none());
    assert!(!job.path().join("result.json").exists());
}

#[test]
fn failed_attempt_archive_io_keeps_the_original_discarded_error_and_proof() {
    let job = tempfile::tempdir().unwrap();
    std::fs::create_dir(job.path().join("attempt-0.json")).unwrap();
    let verification = json!({"status":"PASS","scope":"synthetic lifecycle only"});
    let proof = _4_simulate::control::evidence(
        &json!({"attempt":0}),
        &json!({"observed":true}),
        &verification,
    );
    let mut receipt = json!({"phases":[],"controlVerification":verification,"controlEvidence":proof,"verification":null});
    let mut missing = _4_simulate::control::attach(
        Error::new("NEEDS_INPUT", "requested phase exact input absent")
            .with_details(json!({"schema":"svm-sysvar-discovery/v1","status":"NEEDS_INPUT"})),
        Some(&proof),
    );
    let error = archive_failed_attempt(job.path(), &mut receipt, 0, &mut missing).unwrap_err();
    let details = error
        .details
        .as_ref()
        .expect("failed archive retains attempted evidence");
    assert_eq!(details["failedAttempt"]["error"]["code"], "NEEDS_INPUT");
    assert_eq!(
        details["failedAttempt"]["error"]["message"],
        "requested phase exact input absent"
    );
    assert_eq!(details["failedAttempt"]["controlEvidence"], proof);
    assert_eq!(
        details["failedAttempt"]["error"]["details"]["controlEvidence"],
        proof
    );
    assert_eq!(
        details["failedAttempt"]["disposition"],
        "DISCARDED_NEEDS_INPUT"
    );
    assert_eq!(details["failedAttempt"]["partialResultPublished"], false);
    assert_eq!(details["archiveFile"], "attempt-0.json");
    let final_receipt = finish_job(Err(error), receipt, job.path(), Instant::now()).unwrap();
    assert_eq!(final_receipt["outcome"], "ERROR");
    assert!(final_receipt["controlVerification"].is_null());
    assert!(final_receipt.get("controlEvidence").is_none());
    assert!(final_receipt.get("output").is_none());
    assert!(!job.path().join("result.json").exists());
}
