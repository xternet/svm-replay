use super::*;

fn load_failure(error: &str) -> (Value, Value, Value, Value) {
    let (candidate, fixture, mut block, mut output) = context();
    let meta = &mut block["result"]["transactions"][0]["meta"];
    meta["err"] = json!(error);
    meta["innerInstructions"] = Value::Null;
    meta["logMessages"] = Value::Null;
    meta["computeUnitsConsumed"] = json!(0);
    let execution = &mut output["original"];
    execution["status"] = json!("err");
    execution["normalizedError"] = json!(error);
    execution["logs"] = json!([]);
    execution["computeUnits"] = json!(0);
    (candidate, fixture, block, output)
}

#[test]
fn load_failure_null_recordings_verify_with_exact_fee_and_balances() {
    for error in [
        "MaxLoadedAccountsDataSizeExceeded",
        "ProgramAccountNotFound",
    ] {
        let (candidate, fixture, block, mut output) = load_failure(error);
        verify(
            &candidate,
            &fixture,
            &block,
            &output,
            MetadataPolicy::Strict,
        )
        .unwrap();
        output["original"]["instructionTrace"] = json!({
            "schema":"svm-inner-instructions/v1", "source":"runtime",
            "coverage":"recorded-invocations", "groups":[]});
        verify(
            &candidate,
            &fixture,
            &block,
            &output,
            MetadataPolicy::Strict,
        )
        .unwrap();
        output["original"]["fee"] = json!("6");
        assert!(verify(
            &candidate,
            &fixture,
            &block,
            &output,
            MetadataPolicy::Strict
        )
        .is_err());
    }
}

#[test]
fn load_failure_does_not_exempt_conflicting_or_missing_evidence() {
    for mutation in [
        "cu",
        "logs",
        "return",
        "missing-cpi",
        "missing-logs",
        "other-error",
    ] {
        let (candidate, fixture, mut block, output) = load_failure("ProgramAccountNotFound");
        let meta = &mut block["result"]["transactions"][0]["meta"];
        match mutation {
            "cu" => meta["computeUnitsConsumed"] = json!(1),
            "logs" => meta["logMessages"] = json!(["invoked"]),
            "return" => {
                meta["returnData"] = json!({"programId":"program","data":["AQ==","base64"]})
            }
            "missing-cpi" => {
                meta.as_object_mut().unwrap().remove("innerInstructions");
            }
            "missing-logs" => {
                meta.as_object_mut().unwrap().remove("logMessages");
            }
            "other-error" => meta["err"] = json!("AccountNotFound"),
            _ => unreachable!(),
        }
        assert!(
            verify(
                &candidate,
                &fixture,
                &block,
                &output,
                MetadataPolicy::Strict
            )
            .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn status_mismatch_reports_observed_runtime_error() {
    let (candidate, fixture, block, mut output) = context();
    output["original"]["status"] = json!("err");
    output["original"]["normalizedError"] =
        json!("{\"InstructionError\":[2,\"InvalidAccountData\"]}");
    let error = verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
    )
    .unwrap_err();
    assert!(error.to_string().contains("InvalidAccountData"), "{error}");
}
