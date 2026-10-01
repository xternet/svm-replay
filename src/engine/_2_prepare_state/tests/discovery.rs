use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use svm_replay_engine::_2_prepare_state::context::available_generic_sysvars;
use svm_replay_engine::_2_prepare_state::discovery::{add_input, initial_inputs, tracked_fixture};
use svm_replay_engine::shared::history::CLOCK;

#[path = "discovery_cases/slot_hashes.rs"]
mod slot_hashes;

fn fixture() -> serde_json::Value {
    let mut bytes = [0u8; 40];
    bytes[..8].copy_from_slice(&20u64.to_le_bytes());
    json!({"target":{"targetSlot":20,"parentSlot":19},"accounts":[],"clock":{"pubkey":CLOCK,
      "sourceSlot":20,"role":"sysvar","presence":"present","owner":"Sysvar1111111111111111111111111111111111111",
      "executable":false,"lamports":"1","rentEpoch":"18446744073709551615","dataBase64":STANDARD.encode(bytes)},
      "runtime":{"binding":{"bindingHash":"binding"},"lastRestartSlot":{"bindingHash":"restart"},
        "bankContext":{"targetSlot":20,"parentSlot":19,"executionBlockhash":"block","lamportsPerSignature":5,
          "sourceKind":"kind","blockSourceHash":"hash","blockLineageHash":"lineage","feeWitnessCount":1,
          "feeWitnessIndicesHash":"indices","feeWitnessHash":"witness","requiredRuntimeSysvars":[]}}})
}
#[test]
fn exact_clock_is_declared_and_profile_changes_without_mutating_baseline() {
    let base = fixture();
    let saved = base.clone();
    let inputs = initial_inputs(&base).unwrap();
    let tracked = tracked_fixture(&base, &inputs).unwrap();
    assert_eq!(base, saved);
    assert_eq!(tracked["runtime"]["genericSysvars"][0]["pubkey"], CLOCK);
    assert_eq!(available_generic_sysvars(&tracked).unwrap().len(), 1);
    assert_eq!(
        tracked["runtime"]["profileHash"].as_str().unwrap().len(),
        64
    );
    assert_eq!(
        tracked_fixture(&tracked, &initial_inputs(&tracked).unwrap()).unwrap(),
        tracked
    );
}
#[test]
fn missing_input_requires_exact_target_state_and_proven_progress() {
    let base = fixture();
    let mut inputs = initial_inputs(&base).unwrap();
    let mut rent = base["clock"].clone();
    let key = "SysvarRent111111111111111111111111111111111";
    rent["pubkey"] = json!(key);
    rent["dataBase64"] = json!(STANDARD.encode([0u8; 17]));
    let mut stale = rent.clone();
    stale["sourceSlot"] = json!(19);
    assert!(add_input(&mut inputs, key, stale, 20).is_err());
    add_input(&mut inputs, key, rent.clone(), 20).unwrap();
    assert!(add_input(&mut inputs, key, rent, 20).is_err());
    assert_eq!(
        available_generic_sysvars(&tracked_fixture(&base, &inputs).unwrap())
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn supplied_binding_corruption_and_mixed_modes_reject() {
    let base = fixture();
    let mut tracked = tracked_fixture(&base, &initial_inputs(&base).unwrap()).unwrap();
    tracked["runtime"]["genericSysvars"][0]["dataLen"] = json!(41);
    assert!(initial_inputs(&tracked).is_err());
    let mut base = base;
    base["runtime"]["bankContext"]["requiredRuntimeSysvars"] =
        json!([{"pubkey":CLOCK,"requirement":"complete-generic-sysvar-context-v1"}]);
    assert!(tracked_fixture(&base, &initial_inputs(&base).unwrap()).is_err());
}

#[test]
fn requested_discovery_restarts_the_entire_control_attempt() {
    use svm_replay_engine::_2_prepare_state::discovery::settle;
    use svm_replay_protocol::Error;
    let rent = "SysvarRent111111111111111111111111111111111";
    let base = fixture();
    let mut phases = Vec::new();
    let mut hydrated = Vec::new();
    let result = settle(
        &base,
        |current| {
            phases.push("control");
            phases.push("requested");
            if !available_generic_sysvars(current)?.contains(rent) {
                return Err(
                    Error::new("NEEDS_INPUT", "requested read").with_details(json!({
                "schema":"svm-sysvar-discovery/v1","status":"NEEDS_INPUT","pubkey":rent,
                "reads":[{"pubkey":rent,"offset":"0","length":"17"}]})),
                );
            }
            Ok(json!({"finished":true}))
        },
        |key, slot| {
            hydrated.push((key.to_owned(), slot));
            let mut account = base["clock"].clone();
            account["pubkey"] = json!(key);
            account["dataBase64"] = json!(STANDARD.encode([0u8; 17]));
            Ok(account)
        },
    )
    .unwrap();
    assert_eq!(phases, vec!["control", "requested", "control", "requested"]);
    assert_eq!(hydrated, vec![(rent.to_owned(), 20)]);
    assert_eq!(result.attempts.len(), 1);
    assert_eq!(result.output, json!({"finished":true}));
    assert_eq!(available_generic_sysvars(&result.fixture).unwrap().len(), 2);
    assert_eq!(base["accounts"], json!([]));
}

#[test]
fn discovery_never_recovers_mismatch_partial_output_or_stale_input() {
    use svm_replay_engine::_2_prepare_state::discovery::settle;
    use svm_replay_protocol::Error;
    let base = fixture();
    for code in [
        "MISMATCH",
        "WORKER_TIMEOUT",
        "UNSUPPORTED_HISTORICAL_EPOCH_STAKE",
    ] {
        let error = settle(
            &base,
            |_| Err(Error::new(code, "stop")),
            |_, _| panic!("must not hydrate"),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, code);
    }
    let rent = "SysvarRent111111111111111111111111111111111";
    let incomplete = json!({"schema":"svm-sysvar-discovery/v1","status":"NEEDS_INPUT","pubkey":rent,
        "reads":[{"pubkey":rent,"offset":"0","length":"17"}]});
    let mut partial = incomplete.clone();
    partial["output"] = json!({"fake":"result"});
    assert!(settle(
        &base,
        |_| Err(Error::new("NEEDS_INPUT", "stop").with_details(partial.clone())),
        |_, _| panic!("must not hydrate")
    )
    .is_err());
    assert!(settle(
        &base,
        |_| Err(Error::new("NEEDS_INPUT", "stop").with_details(incomplete.clone())),
        |key, _| {
            let mut account = base["clock"].clone();
            account["pubkey"] = json!(key);
            account["sourceSlot"] = json!(19);
            Ok(account)
        }
    )
    .is_err());
}
