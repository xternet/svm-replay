use super::*;

#[test]
fn generic_availability_requires_target_bytes_bindings_and_valid_declared_modes() {
    let fixture = discovery_fixture();
    for mutation in [
        "wrong-hash",
        "duplicate-binding",
        "extra-binding-field",
        "null-bindings",
        "clock-slot",
        "clock-owner",
        "duplicate-requirement",
        "unknown-requirement",
        "incomplete-complete",
        "mixed-modes",
    ] {
        let mut value = fixture.clone();
        match mutation {
            "wrong-hash" => {
                value["runtime"]["genericSysvars"][0]["dataSha256"] = json!(hash(b"wrong"))
            }
            "duplicate-binding" => {
                let copy = value["runtime"]["genericSysvars"][0].clone();
                value["runtime"]["genericSysvars"]
                    .as_array_mut()
                    .unwrap()
                    .push(copy);
            }
            "extra-binding-field" => value["runtime"]["genericSysvars"][0]["extra"] = json!(1),
            "null-bindings" => value["runtime"]["genericSysvars"] = Value::Null,
            "clock-slot" => value["clock"]["sourceSlot"] = json!(19),
            "clock-owner" => value["clock"]["owner"] = json!(key(5)),
            "duplicate-requirement" => {
                let copy = value["runtime"]["bankContext"]["requiredRuntimeSysvars"][0].clone();
                value["runtime"]["bankContext"]["requiredRuntimeSysvars"]
                    .as_array_mut()
                    .unwrap()
                    .push(copy);
            }
            "unknown-requirement" => {
                value["runtime"]["bankContext"]["requiredRuntimeSysvars"][0]["requirement"] =
                    json!("unknown")
            }
            "incomplete-complete" => {
                value["runtime"]["bankContext"]["requiredRuntimeSysvars"][0]["requirement"] =
                    json!("complete-generic-sysvar-context-v1")
            }
            "mixed-modes" => {
                let mut copy = value["runtime"]["bankContext"]["requiredRuntimeSysvars"][0].clone();
                copy["requirement"] = json!("complete-generic-sysvar-context-v1");
                value["runtime"]["bankContext"]["requiredRuntimeSysvars"]
                    .as_array_mut()
                    .unwrap()
                    .push(copy);
            }
            _ => unreachable!(),
        }
        assert!(available_generic_sysvars(&value).is_err(), "{mutation}");
    }
    let mut legacy = fixture.clone();
    legacy["runtime"]["bankContext"]["requiredRuntimeSysvars"] = json!([]);
    legacy["runtime"]
        .as_object_mut()
        .unwrap()
        .remove("genericSysvars");
    assert!(available_generic_sysvars(&legacy).unwrap().is_empty());
}

#[test]
fn full_generic_context_requires_all_seven_bound_accounts() {
    use svm_replay_engine::_2_prepare_state::context::GENERIC_SYSVARS;
    let mut value = discovery_fixture();
    for id in &GENERIC_SYSVARS[1..] {
        let data = STANDARD.encode([1, 2, 3]);
        value["accounts"].as_array_mut().unwrap().push(json!({"pubkey":id,"sourceSlot":20,"role":"sysvar",
            "presence":"present","owner":"Sysvar1111111111111111111111111111111111111","executable":false,"dataBase64":data}));
        value["runtime"]["genericSysvars"]
            .as_array_mut()
            .unwrap()
            .push(json!({"pubkey":id,"sourceSlot":20,"dataSha256":hash([1,2,3]),"dataLen":3}));
    }
    value["runtime"]["bankContext"]["requiredRuntimeSysvars"] = json!(GENERIC_SYSVARS
        .iter()
        .map(|id| json!({"pubkey":id,"requirement":"complete-generic-sysvar-context-v1"}))
        .collect::<Vec<_>>());
    assert_eq!(
        available_generic_sysvars(&value).unwrap().len(),
        GENERIC_SYSVARS.len()
    );
    let before = value.clone();
    value["runtime"]["genericSysvars"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(available_generic_sysvars(&value).is_err());
    let mut stale = before.clone();
    stale["accounts"][1]["sourceSlot"] = json!(19);
    assert!(available_generic_sysvars(&stale).is_err());
    let mut duplicate = before;
    let copy = duplicate["accounts"][1].clone();
    duplicate["accounts"].as_array_mut().unwrap().push(copy);
    assert!(available_generic_sysvars(&duplicate).is_err());
}
