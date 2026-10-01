use super::*;

#[test]
fn exact_integer_evidence_verifies_without_mutating_output() {
    let (candidate, fixture, block, output) = context();
    let before = output.clone();
    let report = verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
    )
    .unwrap();
    let report = serde_json::to_value(report).unwrap();
    assert_eq!(report["status"], "PASS");
    assert_eq!(report["original"]["immediateTokenAmounts"], 1);
    assert_eq!(output, before);
}

#[test]
fn incomplete_archived_evidence_never_matches() {
    for field in [
        "err",
        "innerInstructions",
        "logMessages",
        "computeUnitsConsumed",
        "fee",
        "preBalances",
        "postBalances",
        "preTokenBalances",
        "postTokenBalances",
    ] {
        for delete in [true, false] {
            if field == "err" && !delete {
                continue;
            }
            let (candidate, fixture, mut block, output) = context();
            let meta = block["result"]["transactions"][0]["meta"]
                .as_object_mut()
                .unwrap();
            if delete {
                meta.remove(field);
            } else {
                meta.insert(field.into(), Value::Null);
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
                "{field}, delete={delete}"
            );
        }
    }
}

#[test]
fn omitted_and_empty_archival_return_data_mean_no_return() {
    for data in [
        Value::Null,
        json!({"programId":"program","data":["","base64"]}),
    ] {
        let (candidate, fixture, mut block, output) = context();
        block["result"]["transactions"][0]["meta"]["returnData"] = data;
        verify(
            &candidate,
            &fixture,
            &block,
            &output,
            MetadataPolicy::Strict,
        )
        .unwrap();
    }
    let (candidate, fixture, mut block, mut output) = context();
    block["result"]["transactions"][0]["meta"]["returnData"] =
        json!({"programId":"program","data":["%%%","base64"]});
    output["original"]["returnData"] = json!({"programId":"program","dataBase64":"%%%"});
    assert!(verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict
    )
    .is_err());
}

#[test]
fn token_metadata_requires_unique_in_range_indices_and_existing_dimensions() {
    for field in ["preTokenBalances", "postTokenBalances"] {
        for mutation in ["duplicate", "out-of-range", "empty"] {
            let (candidate, fixture, mut block, output) = context();
            let tokens = block["result"]["transactions"][0]["meta"][field]
                .as_array_mut()
                .unwrap();
            match mutation {
                "duplicate" => tokens.push(tokens[0].clone()),
                "out-of-range" => {
                    tokens.push(json!({"accountIndex":2,"uiTokenAmount":{"amount":"0"}}))
                }
                "empty" => tokens.clear(),
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
                "{field} {mutation}"
            );
        }
    }
}

#[test]
fn identity_and_immediate_evidence_cannot_be_removed_or_changed() {
    for mutation in [
        "slot",
        "parent",
        "index",
        "signature",
        "profile",
        "duplicate-end",
        "end-slot",
        "deleted-evidence",
        "presence",
        "balance",
        "end-data",
    ] {
        let (candidate, mut fixture, block, mut output) = context();
        match mutation {
            "slot" => fixture["target"]["targetSlot"] = json!(21),
            "parent" => fixture["target"]["parentSlot"] = json!(18),
            "index" => fixture["target"]["index"] = json!(1),
            "signature" => fixture["target"]["signature"] = json!("other"),
            "profile" => fixture["runtime"]["binding"]["runtimeProfileId"] = json!("other"),
            "duplicate-end" => {
                let copy = fixture["endAccounts"][0].clone();
                fixture["endAccounts"].as_array_mut().unwrap().push(copy);
            }
            "end-slot" => fixture["endAccounts"][0]["sourceSlot"] = json!(19),
            "deleted-evidence" => {
                fixture["target"]["evidenceAccounts"] = json!([]);
                output["original"]["accountTransitions"] = json!([]);
            }
            "presence" => {
                output["original"]["accountTransitions"][0]["before"]
                    .as_object_mut()
                    .unwrap()
                    .remove("presence");
            }
            "balance" => {
                output["original"]["accountTransitions"][0]["after"]["lamports"] =
                    json!("9007199254740987")
            }
            "end-data" => {
                output["original"]["endAccountStates"][0]["state"]["dataHash"] =
                    json!(hash([1, 2, 4]))
            }
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
