use super::*;
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use svm_replay_engine::shared::{
    history::History,
    runtime::{CancellationToken, ExecutionBudget},
    sources::{CompositeSource, HistoricalSource, SourceError},
};
use svm_replay_protocol::Digest;

struct Source {
    token: CancellationToken,
    cancel: bool,
    delay: bool,
    fail: bool,
    calls: Arc<AtomicUsize>,
}
impl HistoricalSource for Source {
    fn identity(&self) -> Value {
        json!({"id":"budget-test","version":"1","kind":"captured-history","genesisHash":"11111111111111111111111111111111",
            "coverage":{"firstSlot":0,"lastSlot":100,"completeness":"partial"},"capabilities":["account","block"]})
    }
    fn inspect(&self, query: &Value) -> Result<Option<Value>, SourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.cancel {
            self.token.cancel();
        }
        if self.delay {
            std::thread::sleep(Duration::from_millis(150));
        }
        if self.fail {
            return Err(SourceError::new(
                "SOURCE_TRANSPORT",
                "controlled source failure",
            ));
        }
        Ok(Some(
            json!({"query":query,"value":account(),"evidenceHashes":[Digest::of(b"controlled as-of account")]}),
        ))
    }
    fn discover_block(&self, slot: u64) -> Result<Option<Value>, SourceError> {
        self.inspect(&json!({"kind":"account"}))?;
        let raw = serde_json::to_vec(&json!({"result":{"parentSlot":slot-1,
            "blockhash":"11111111111111111111111111111111","transactions":[]}}))
        .unwrap();
        Ok(Some(json!({"query":{"kind":"block","slot":slot,
            "genesisHash":"11111111111111111111111111111111","blockEvidenceSha256":Digest::of(&raw)},
            "value":{"rawBase64":STANDARD.encode(&raw)},"evidenceHashes":[Digest::of(&raw)]})))
    }
}
fn run(
    cancel_before: bool,
    cancel_during: bool,
    timeout: bool,
    fail: bool,
) -> (Result<Value, svm_replay_protocol::Error>, usize) {
    let token = CancellationToken::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut sources = CompositeSource::new(vec![Box::new(Source {
        token: token.clone(),
        cancel: cancel_during,
        delay: timeout,
        fail,
        calls: calls.clone(),
    })])
    .unwrap();
    let budget = ExecutionBudget::new(
        if timeout {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(5)
        },
        token.clone(),
    )
    .unwrap();
    if cancel_before {
        token.cancel();
    }
    let mut history = History::new(
        &mut sources,
        "11111111111111111111111111111111".into(),
        100,
        &budget,
        1,
    )
    .unwrap();
    let result = history.read(
        &json!({"kind":"account","genesisHash":"11111111111111111111111111111111",
        "slot":100,"pubkey":"11111111111111111111111111111111","phase":"end-slot"}),
    );
    (result, calls.load(Ordering::SeqCst))
}
#[test]
fn cancellation_after_source_return_dominates_success_and_preserves_provider_failure() {
    for fail in [false, true] {
        let (result, calls) = run(false, true, false, fail);
        let error = result.expect_err("late source return must not bypass cancellation");
        assert_eq!(error.code, "WORKER_CANCELLED");
        assert_eq!(calls, 1);
        if fail {
            assert_eq!(
                error.details.unwrap()["sourceError"]["code"],
                "SOURCE_TRANSPORT"
            );
        }
    }
    let (result, calls) = run(true, false, false, false);
    assert_eq!(result.unwrap_err().code, "WORKER_CANCELLED");
    assert_eq!(calls, 0);
}
#[test]
fn expired_job_budget_dominates_late_source_success_and_error() {
    for fail in [false, true] {
        let (result, calls) = run(false, false, true, fail);
        let error = result.expect_err("late source return must not bypass deadline");
        assert_eq!(error.code, "WORKER_TIMEOUT");
        assert_eq!(calls, 1);
        if fail {
            assert_eq!(
                error.details.unwrap()["sourceError"]["code"],
                "SOURCE_TRANSPORT"
            );
        }
    }
    let (result, calls) = run(false, false, false, true);
    assert_eq!(result.unwrap_err().code, "SOURCE_TRANSPORT");
    assert_eq!(calls, 1);
}

#[test]
fn discovery_shares_cancellation_and_read_budget_with_pinned_queries() {
    for cancel in [false, true] {
        let token = CancellationToken::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut sources = CompositeSource::new(vec![Box::new(Source {
            token: token.clone(),
            cancel,
            delay: false,
            fail: false,
            calls: calls.clone(),
        })])
        .unwrap();
        let budget = ExecutionBudget::new(Duration::from_secs(5), token).unwrap();
        let mut history = History::new(
            &mut sources,
            "11111111111111111111111111111111".into(),
            100,
            &budget,
            1,
        )
        .unwrap();
        let result = history.discover_block(99);
        if cancel {
            assert_eq!(result.unwrap_err().code, "WORKER_CANCELLED");
        } else {
            assert!(result.is_ok());
            assert_eq!(
                history.discover_block(98).unwrap_err().code,
                "UNSUPPORTED_RESOURCE_LIMIT"
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
