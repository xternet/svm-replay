//! Explicit real-history qualification; no provider or synthesized transaction.
use crate::{unix::inputs, verified::reviewed_input};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
use svm_replay_engine::shared::{
    bank::{context, sysvars},
    historical::{NativeSession, SessionCommand},
    runtime::{CancellationToken, ExecutionBudget, WorkerLimits, WorkerTransport},
};
use svm_replay_protocol::{Digest, PreparedRequest};

#[test]
#[ignore = "requires reviewed real SVM_SESSION_FIXTURE/RECEIPT/WORKER pins; offline integration driver"]
fn real_initialized_slot_starts_before_first_vote_without_an_archived_control() {
    let (mut fixture, worker, owner) = inputs();
    let descriptor = reviewed_input(worker.executable.clone());
    let raw = std::fs::read(&descriptor.receipt_path).unwrap();
    assert_eq!(Digest::of(&raw).as_str(), descriptor.receipt_sha256);
    let receipt: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(receipt["verification"]["status"], "PASS");
    let result_path = descriptor
        .receipt_path
        .parent()
        .unwrap()
        .join(receipt["output"]["file"].as_str().unwrap());
    let raw = std::fs::read(result_path).unwrap();
    assert_eq!(
        Digest::of(&raw).as_str(),
        receipt["output"]["sha256"].as_str().unwrap()
    );
    let result: Value = serde_json::from_slice(&raw).unwrap();
    let prepared_path = descriptor
        .receipt_path
        .parent()
        .unwrap()
        .join(receipt["preparedRequest"]["file"].as_str().unwrap());
    let raw = std::fs::read(prepared_path).unwrap();
    assert_eq!(
        Digest::of(&raw).as_str(),
        receipt["preparedRequest"]["sha256"].as_str().unwrap()
    );
    let prepared = PreparedRequest::parse(&raw).unwrap();
    assert_eq!(fixture, prepared.fixture);
    for name in [
        "systemCreditPreloads",
        "programHeaderProofs",
        "programMigrationProofs",
        "stakeInitializationProofs",
    ] {
        assert!(fixture["runtime"].get(name).is_none_or(|v| v.is_null() || v.as_array().is_some_and(Vec::is_empty)),
            "this real initial-image oracle must not contain transaction-dependent preloads: {name}");
    }
    assert!(fixture["runtime"].get("initializedStakeSnapshot").is_none());
    let slot = fixture["target"]["targetSlot"].as_u64().unwrap();
    let block = prepared.archived_block().unwrap();
    let first = &block["result"]["transactions"][0]["transaction"]["message"];
    assert!(
        first["instructions"].as_array().unwrap().iter().any(|ix| {
            first["accountKeys"][ix["programIdIndex"].as_u64().unwrap() as usize]
                == "Vote111111111111111111111111111111111111111"
        }),
        "qualify the actual vote-first block, not an easier substituted boundary"
    );
    let mut bank =
        context::derive_initialized_bank_context(&block, slot, prepared.block_sha256.as_str())
            .unwrap();
    bank["requiredRuntimeSysvars"] =
        fixture["runtime"]["bankContext"]["requiredRuntimeSysvars"].clone();
    bank["derivationHash"] = json!(context::historical_bank_context_hash(&bank).unwrap());
    assert_eq!(bank, fixture["runtime"]["bankContext"]);
    let parent = block["result"]["parentSlot"].as_u64().unwrap();
    let accounts = fixture["accounts"].as_array().unwrap();
    sysvars::assert_bank_initialized_sysvar_bindings(
        accounts,
        slot,
        fixture["runtime"]["bankInitializedSysvars"]
            .as_array()
            .unwrap(),
    )
    .unwrap();
    for account in accounts {
        if !sysvars::BANK_INITIALIZED_SYSVARS.contains(&account["pubkey"].as_str().unwrap()) {
            assert_eq!(account["sourceSlot"], parent);
            assert_ne!(
                account["owner"],
                "Stake11111111111111111111111111111111111111"
            );
        }
    }
    let recorded_index = fixture["target"]["index"].as_u64().unwrap() as usize;
    let recorded = &block["result"]["transactions"][recorded_index];
    let payer = recorded["transaction"]["message"]["accountKeys"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let image = accounts.iter().find(|a| a["pubkey"] == payer).unwrap();
    let balance = image["lamports"].as_str().unwrap().parse::<u64>().unwrap();
    assert_eq!(recorded["meta"]["preBalances"][0], balance);
    let wire = fixture["target"]["transactionBase64"]
        .as_str()
        .unwrap()
        .to_owned();
    let signature = fixture["target"]["signature"].as_str().unwrap().to_owned();
    fixture["schema"] = json!("svm-historical-initialized-slot/v1");
    fixture["target"] = json!({"kind":"initialized-slot","targetSlot":slot,"index":0});
    fixture["endAccounts"] = json!([]);
    fixture["runtime"]["afterSlot"] = json!(parent);
    let scratch = tempfile::tempdir().unwrap();
    let budget = ExecutionBudget::new(Duration::from_secs(120), CancellationToken::new()).unwrap();
    let mut session = NativeSession::open(
        WorkerTransport::new(scratch.path()).with_owner(owner),
        worker,
        fixture,
        false,
        WorkerLimits::default(),
        budget.clone(),
    )
    .unwrap();
    assert!(session.ready()["original"].is_null());
    assert_eq!(
        session.ready()["originalControl"]["status"],
        "NOT_APPLICABLE"
    );
    let records = block["result"]["transactions"].as_array().unwrap().iter().enumerate()
        .map(|(index, tx)| json!({"index":index,"transactionBase64":svm_replay_protocol::transaction::encode(tx).unwrap(),"err":tx["meta"]["err"]}))
        .collect::<Vec<_>>();
    let initial = session.request(SessionCommand::ConfigureRpc { context: json!({
        "schema":"svm-native-rpc-context/v1","sourceHash":prepared.block_sha256,
        "slot":slot,"transactionIndex":0,"blockHeight":block["result"]["blockHeight"],"transactions":records}) }, &budget).unwrap();
    assert_eq!(initial["transactionIndex"], 0);
    let values = session
        .request(
            SessionCommand::Accounts {
                pubkeys: vec![payer.clone()],
            },
            &budget,
        )
        .unwrap();
    assert_eq!(
        values[0]["lamports"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        balance
    );
    let statuses = || SessionCommand::SignatureStatuses {
        signatures: vec![signature.clone()],
    };
    assert!(session.request(statuses(), &budget).unwrap()[0].is_null());
    let sent = session
        .request(
            SessionCommand::RpcSend {
                transaction_base64: wire,
                skip_preflight: true,
            },
            &budget,
        )
        .unwrap();
    assert_eq!(sent["admitted"], true);
    assert_eq!(sent["output"]["status"], result["original"]["status"]);
    assert_eq!(
        sent["output"]["computeUnits"],
        result["original"]["computeUnits"]
    );
    assert_eq!(sent["output"]["logs"], result["original"]["logs"]);
    let status = session.request(statuses(), &budget).unwrap();
    // The standalone worker finalizes a committed local write immediately.
    assert_eq!(status[0]["confirmationStatus"], "finalized");
    assert!(status[0]["confirmations"].is_null());
    assert_eq!(status[0]["slot"], slot);
    let after = session
        .request(
            SessionCommand::Accounts {
                pubkeys: vec![payer],
            },
            &budget,
        )
        .unwrap();
    assert_eq!(
        after[0]["lamports"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        recorded["meta"]["postBalances"][0]
    );
    assert_eq!(
        session.request(SessionCommand::Reset, &budget).unwrap()["stateSha256"],
        initial["stateSha256"]
    );
    assert!(session.request(statuses(), &budget).unwrap()[0].is_null());
    session.close().unwrap();
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
    assert!(Path::new(&descriptor.receipt_path).is_file());
}
