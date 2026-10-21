use super::*;
use crate::shared::{
    runtime::{CancellationToken, ExecutionBudget},
    sources::{CompositeSource, HistoricalSource, SourceError},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const ACCOUNT: &str = "11111111111111111111111111111111";

struct Accounts {
    missing: bool,
    calls: Arc<Mutex<Vec<String>>>,
}

impl HistoricalSource for Accounts {
    fn identity(&self) -> Value {
        json!({"id":"credit-acquisition-test","version":"1","kind":"captured-history",
            "genesisHash":ACCOUNT,"coverage":{"firstSlot":9,"lastSlot":10,"completeness":"partial"},
            "capabilities":["account"]})
    }

    fn inspect(&self, query: &Value) -> Result<Option<Value>, SourceError> {
        let key = query["pubkey"].as_str().unwrap();
        self.calls.lock().unwrap().push(key.into());
        if key != ACCOUNT || self.missing {
            return Ok(None);
        }
        Ok(Some(
            json!({"query":query,"value":{"pubkey":key,"sourceSlot":9,
            "role":"application","presence":"absent"},
            "evidenceHashes":[Digest::of(b"controlled absence at parent slot")]}),
        ))
    }
}

fn run(missing: bool, dependencies: &[&str]) -> (Result<CreditPlan, Error>, Vec<String>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut sources = CompositeSource::new(vec![Box::new(Accounts {
        missing,
        calls: calls.clone(),
    })])
    .unwrap();
    let budget = ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).unwrap();
    let mut history = History::new(&mut sources, ACCOUNT.into(), 10, &budget, 10).unwrap();
    let input: HistoricalRequest = serde_json::from_value(json!({
        "schema":"svm-replay-historical/v1","requestId":"credit-acquisition",
        "family":"v3-1","genesisHash":ACCOUNT,"candidate":{},
        "runtimeBinding":{"executor":{"id":"litesvm-v0.12.0-agave-3.1.11"}},
        "replacementTransactionBase64":null,"requestedAccountOverrides":null,
        "bankInputs":[],"metadataPolicy":"ARCHIVED_COMPUTE_METER_WARNING",
        "limits":{"timeoutMs":5000,"maxOutputBytes":1048576,"maxDiagnosticBytes":1048576}
    }))
    .unwrap();
    let a = Analysis {
        slot: 10,
        parent: 9,
        index: 0,
        blockhash: ACCOUNT.into(),
        raw: vec![json!({"transaction":{"message":{"accountKeys":[ACCOUNT]}},
            "meta":{"preBalances":[0]}})],
        transactions: vec![],
    };
    let closure = dependencies::SemanticClosureResult {
        seed_accounts: vec![ACCOUNT.into()],
        selected_indices: vec![],
        application_dependency_accounts: vec![ACCOUNT.into()],
        dependency_accounts: dependencies.iter().map(|key| (*key).into()).collect(),
        program_data_accounts: vec![],
        excluded_dependency_accounts: vec![],
        fee_only_payers_ignored: vec![],
        included_vote_indices: vec![],
        writable_intersections: vec![],
        iterations: 1,
    };
    let result = recover(&input, &a, &mut history, &[], closure);
    let observed = calls.lock().unwrap().clone();
    (result, observed)
}

#[test]
fn credit_precheck_does_not_acquire_generated_or_initialized_sysvars() {
    let (result, calls) = run(false, &[INSTRUCTIONS, CLOCK, ACCOUNT]);
    let plan = result.unwrap();
    assert_eq!(calls, vec![ACCOUNT]);
    assert!(plan.images.is_empty());
    assert!(plan.proofs.is_empty());
    assert_eq!(
        plan.closure.dependency_accounts,
        vec![INSTRUCTIONS, CLOCK, ACCOUNT]
    );
}

#[test]
fn credit_precheck_still_refuses_unavailable_application_accounts() {
    let (result, calls) = run(true, &[INSTRUCTIONS, CLOCK, ACCOUNT]);
    assert_eq!(result.err().unwrap().code, "SOURCE_UNAVAILABLE");
    assert_eq!(calls, vec![ACCOUNT]);
}

#[test]
fn credit_precheck_leaves_recent_blockhashes_to_the_guarded_parent_pipeline() {
    let (result, calls) = run(false, &[RECENT_BLOCKHASHES, ACCOUNT]);
    let plan = result.unwrap();
    assert_eq!(calls, vec![ACCOUNT]);
    assert!(plan.images.is_empty());
    assert!(plan.proofs.is_empty());
    // The precheck cannot initialize this sysvar or authorize its omission.
    // Only the existing nonce-use proof can exclude it from parent acquisition.
    let required = parent_keys(&plan.closure, &[], &BTreeSet::new()).unwrap();
    assert!(required.iter().any(|key| key == RECENT_BLOCKHASHES));
    let resolvable = BTreeSet::from([RECENT_BLOCKHASHES.into()]);
    let supplied_by_runtime = parent_keys(&plan.closure, &[], &resolvable).unwrap();
    assert!(!supplied_by_runtime
        .iter()
        .any(|key| key == RECENT_BLOCKHASHES));
}
