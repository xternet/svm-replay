use super::*;

#[test]
fn normalized_error_wire_never_aliases_a_different_or_debug_error() {
    let (candidate, fixture, mut block, mut output) = context();
    block["result"]["transactions"][0]["meta"]["err"] =
        json!({"InstructionError":[7,"UnsupportedProgramId"]});
    output["original"]["status"] = json!("err");
    output["original"]["normalizedError"] =
        json!(r#"{"InstructionError":[7,"UnsupportedProgramId"]}"#);
    verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
    )
    .unwrap();
    for error in [
        r#"{"InstructionError":[7,"UnknownFutureError"]}"#,
        r#"{"InstructionError":[8,"UnsupportedProgramId"]}"#,
        r#"{"InstructionError":[7,{"Custom":6000}]}"#,
        r#""UnsupportedProgramId""#,
        r#"{"InstructionError":[7,"UnsupportedProgramId"]"#,
        "InstructionError(7, UnsupportedProgramId)",
        "null",
    ] {
        output["original"]["normalizedError"] = json!(error);
        assert!(
            verify(
                &candidate,
                &fixture,
                &block,
                &output,
                MetadataPolicy::Strict
            )
            .is_err(),
            "{error}"
        );
    }
}

#[test]
fn unsupported_policy_malformed_integers_and_required_trace_fail_closed() {
    assert!("UNKNOWN".parse::<MetadataPolicy>().is_err());
    let (candidate, mut fixture, block, output) = context();
    fixture["runtime"]["binding"]["executor"]["m9Build"] =
        json!({"capabilities":["runtime-instruction-trace/v1"]});
    assert!(verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict
    )
    .is_err());
    for raw in [
        "18446744073709551616",
        "-1",
        "1.5",
        "1e1",
        "\"01\"",
        "\"+1\"",
    ] {
        let value: Value = serde_json::from_str(raw).unwrap();
        assert!(
            svm_replay_engine::shared::diff::exact_u64(&value).is_err(),
            "{raw}"
        );
    }
    let value: Value = serde_json::from_str("18446744073709551615").unwrap();
    assert_eq!(
        svm_replay_engine::shared::diff::exact_u64(&value).unwrap(),
        u64::MAX
    );
}

#[test]
fn requested_override_proof_checks_before_state_and_hashes_actual_request() {
    let (candidate, mut fixture, block, mut output) = context();
    fixture["requestedAccountOverrides"] =
        json!([{"pubkey":"payer","lamports":"9","dataBase64":"BAU="}]);
    output["requestedOverrides"] = output["original"].clone();
    output["requestedOverrides"]["accountTransitions"][0]["before"]["lamports"] = json!("9");
    output["requestedOverrides"]["accountTransitions"][0]["before"]["dataHash"] =
        json!(hash([4, 5]));
    let report = verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
    )
    .unwrap();
    let proof = &report.0["requestedOverrides"];
    assert_eq!(proof["accounts"], 1);
    assert_eq!(proof["payloadSha256"], hash([1]));
    for mutation in [
        "balance",
        "data",
        "duplicate",
        "unknown-field",
        "missing-attempt",
    ] {
        let mut changed_output = output.clone();
        let mut changed_fixture = fixture.clone();
        match mutation {
            "balance" => {
                changed_output["requestedOverrides"]["accountTransitions"][0]["before"]
                    ["lamports"] = json!("8")
            }
            "data" => {
                changed_output["requestedOverrides"]["accountTransitions"][0]["before"]
                    ["dataHash"] = json!(hash([4, 6]))
            }
            "duplicate" => {
                let copy = changed_fixture["requestedAccountOverrides"][0].clone();
                changed_fixture["requestedAccountOverrides"]
                    .as_array_mut()
                    .unwrap()
                    .push(copy);
            }
            "unknown-field" => {
                changed_fixture["requestedAccountOverrides"][0]["owner"] = json!("other")
            }
            "missing-attempt" => changed_output["requestedOverrides"] = Value::Null,
            _ => unreachable!(),
        }
        assert!(
            verify(
                &candidate,
                &changed_fixture,
                &block,
                &changed_output,
                MetadataPolicy::Strict
            )
            .is_err(),
            "{mutation}"
        );
    }
}
