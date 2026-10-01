use super::*;

#[test]
fn warning_policy_only_accepts_explained_meter_changes() {
    let canonical = vec![
        "Program demo consumed 100 of 200 compute units".into(),
        "Program next consumed 20 of 100 compute units".into(),
    ];
    let replay = vec![
        "Program demo consumed 90 of 200 compute units".into(),
        "Program next consumed 20 of 110 compute units".into(),
    ];
    let divergence = compare_archived_compute_metadata("meter", &canonical, &replay, 120, 110)
        .unwrap()
        .unwrap();
    assert_eq!(divergence.different_log_indices, vec![0, 1]);
    assert_eq!(divergence.compute_unit_delta, 10);
    for prefix in [
        "Program log:",
        "Program data:",
        "Program return:",
        "Program log: hello\nProgram demo",
    ] {
        assert!(compare_archived_compute_metadata(
            "text",
            &[format!("{prefix} consumed 100 of 200 compute units")],
            &[format!("{prefix} consumed 90 of 200 compute units")],
            100,
            90
        )
        .is_err());
    }
    assert!(compare_archived_compute_metadata(
        "truncated",
        &["Log truncated".into()],
        &["Log truncated".into()],
        0,
        0
    )
    .is_err());
    let (candidate, fixture, block, mut output) = context();
    output["original"]["computeUnits"] = json!(90);
    output["original"]["logs"][0] = json!("Program demo consumed 90 of 200 compute units");
    assert!(verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict
    )
    .is_err());
    let report = verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::ArchivedComputeMeterWarning,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(report).unwrap()["metadataDivergences"][0]["computeUnitDelta"],
        10
    );
    for (field, changed) in [
        ("status", json!("err")),
        ("fee", json!("6")),
        ("normalizedError", json!("changed")),
        (
            "returnData",
            json!({"programId":"other","dataBase64":"AQ=="}),
        ),
    ] {
        let mut changed_output = output.clone();
        changed_output["original"][field] = changed;
        assert!(
            verify(
                &candidate,
                &fixture,
                &block,
                &changed_output,
                MetadataPolicy::ArchivedComputeMeterWarning
            )
            .is_err(),
            "{field}"
        );
    }
}

#[test]
fn replacement_change_requirement_is_explicit() {
    let (candidate, mut fixture, block, mut output) = context();
    fixture["target"]["replacementTransactionBase64"] =
        fixture["target"]["transactionBase64"].clone();
    output["replacement"] = output["original"].clone();
    assert!(verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict
    )
    .is_err());
    let report = verify_with_options(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
        false,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(report).unwrap()["replacement"],
        json!({"attempted":true,"behaviorChanged":false})
    );
}

#[test]
fn runtime_trace_compares_paths_positions_bytes_and_archived_heights() {
    let (trace, mut archive) = trace_context();
    assert_eq!(
        verify_runtime_instruction_trace(&trace, &archive).unwrap(),
        json!({"status":"MATCH","instructions":3,"stackHeightsVerified":3,"stackHeightsUnavailable":0})
    );
    archive[0]["instructions"][1]["stackHeight"] = Value::Null;
    assert_eq!(
        verify_runtime_instruction_trace(&trace, &archive).unwrap()["stackHeightsUnavailable"],
        1
    );
    for (field, value) in [
        ("dataBase64", json!("AQ==")),
        ("accounts", json!([1, 0])),
        ("programIdIndex", json!(4)),
        ("stackHeight", json!(3)),
        ("path", json!([1, 7])),
    ] {
        let mut changed = trace.clone();
        changed["groups"][0]["instructions"][0][field] = value;
        assert!(
            verify_runtime_instruction_trace(&changed, &archive).is_err(),
            "{field}"
        );
    }
    archive
        .as_array_mut()
        .unwrap()
        .push(json!({"index":1,"instructions":[]}));
    assert!(verify_runtime_instruction_trace(&trace, &archive).is_err());
}

#[test]
fn loaded_addresses_and_cpi_groups_cannot_be_missing_misaligned_or_duplicated() {
    for mutation in [
        "missing-loaded",
        "wrong-loaded-count",
        "duplicate-cpi",
        "cpi-index",
    ] {
        let (candidate, fixture, mut block, output) = context();
        let tx = &mut block["result"]["transactions"][0];
        match mutation {
            "missing-loaded" | "wrong-loaded-count" => {
                tx["transaction"]["message"]["addressTableLookups"] = json!([
                    {"accountKey":"table", "writableIndexes":[0,1], "readonlyIndexes":[]}]);
                if mutation == "wrong-loaded-count" {
                    tx["meta"]["loadedAddresses"] = json!({"writable":["only-one"],"readonly":[]});
                }
            }
            "duplicate-cpi" => {
                tx["meta"]["innerInstructions"] = json!([
                {"index":0,"instructions":[]}, {"index":0,"instructions":[]}])
            }
            "cpi-index" => tx["meta"]["innerInstructions"] = json!([{"index":1,"instructions":[]}]),
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
