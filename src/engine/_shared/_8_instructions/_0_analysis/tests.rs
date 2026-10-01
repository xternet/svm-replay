use super::*;

fn load_failure() -> Value {
    json!({"transaction":{"message":{"accountKeys":["11111111111111111111111111111111"],
        "instructions":[{"programIdIndex":0,"accounts":[0],"data":""}]}},
        "meta":{"loadedAddresses":{"writable":[],"readonly":[]},
        "err":"ProgramAccountNotFound","innerInstructions":null,"logMessages":null,
        "computeUnitsConsumed":0,"returnData":null}})
}

#[test]
fn load_stage_missing_program_has_no_cpi_but_keeps_outer_dependencies() {
    let instructions = resolve(&load_failure()).unwrap();
    assert_eq!(instructions.len(), 1);
    assert!(!instructions[0].inner);
    assert_eq!(
        instructions[0].accounts,
        ["11111111111111111111111111111111"]
    );
}

#[test]
fn missing_program_exception_rejects_conflicting_or_incomplete_execution_evidence() {
    for (field, value) in [
        ("err", Value::Null),
        (
            "err",
            json!({"InstructionError":[0,"ProgramAccountNotFound"]}),
        ),
        ("computeUnitsConsumed", json!(1)),
        ("computeUnitsConsumed", Value::Null),
        ("logMessages", json!(["Program invoked"])),
        (
            "returnData",
            json!({"programId":"11111111111111111111111111111111"}),
        ),
    ] {
        let mut raw = load_failure();
        raw["meta"][field] = value;
        assert!(resolve(&raw).is_err(), "{field}");
    }
    for field in ["innerInstructions", "logMessages", "computeUnitsConsumed"] {
        let mut raw = load_failure();
        raw["meta"].as_object_mut().unwrap().remove(field);
        assert!(resolve(&raw).is_err(), "missing {field}");
    }
}
