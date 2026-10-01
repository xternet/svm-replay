use super::*;

// Opt-in evidence test; this reads immutable recorded executions, not fresh SVM runs.
#[test]
#[ignore = "requires explicit SVM_REPLAY_TEST_VERIFY_M6_ROOT and SVM_REPLAY_TEST_VERIFY_BLOCK_ROOT archived data paths"]
fn preserved_m6_archive_verifies_and_rejects_mutations() {
    let root = std::path::PathBuf::from(
        std::env::var("SVM_REPLAY_TEST_VERIFY_M6_ROOT")
            .expect("SVM_REPLAY_TEST_VERIFY_M6_ROOT required"),
    );
    let blocks = std::path::PathBuf::from(
        std::env::var("SVM_REPLAY_TEST_VERIFY_BLOCK_ROOT")
            .expect("SVM_REPLAY_TEST_VERIFY_BLOCK_ROOT required"),
    );
    let read = |path: &std::path::Path| -> Value {
        serde_json::from_slice(
            &std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
        )
        .unwrap()
    };
    let plan = read(&root.join("m6-plan.json"));
    let candidate = &plan["cases"][0];
    let case_root = root
        .join("cases")
        .join(candidate["id"].as_str().unwrap())
        .join("attempt-1");
    let fixture = read(&case_root.join("fixture.json"));
    let output = read(&case_root.join("executor-output.json"));
    let raw = std::fs::read(blocks.join(candidate["blockFile"].as_str().unwrap())).unwrap();
    assert_eq!(hash(&raw), candidate["blockSourceHash"]);
    let block: Value = serde_json::from_slice(&raw).unwrap();
    let report = verify(candidate, &fixture, &block, &output, MetadataPolicy::Strict).unwrap();
    assert_eq!(report.0["status"], "PASS");
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
        let mut changed = block.clone();
        let index = candidate["transactionIndex"].as_u64().unwrap() as usize;
        changed["result"]["transactions"][index]["meta"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            verify(
                candidate,
                &fixture,
                &changed,
                &output,
                MetadataPolicy::ArchivedComputeMeterWarning
            )
            .is_err(),
            "{field}"
        );
    }
}

#[test]
fn prefix_executions_compare_archive_independently() {
    let (mut candidate, mut fixture, mut block, mut output) = context();
    let copy = block["result"]["transactions"][0].clone();
    block["result"]["transactions"]
        .as_array_mut()
        .unwrap()
        .push(copy);
    candidate["transactionIndex"] = json!(1);
    candidate["rawEvidenceHash"] = json!(hash(format!(
        "{}\n1\nsignature\n",
        candidate["blockSourceHash"].as_str().unwrap()
    )));
    fixture["target"]["index"] = json!(1);
    fixture["target"]["prefixIndices"] = json!([0]);
    fixture["target"]["prefixSignatures"] = json!(["signature"]);
    output["targetIndex"] = json!(1);
    let mut prefix = output["original"].clone();
    prefix["index"] = json!(0);
    prefix["signature"] = json!("signature");
    prefix["endAccountStates"] = json!([]);
    output["prefix"] = json!([prefix]);
    verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
    )
    .unwrap();
    output["prefix"][0]["fee"] = json!("6");
    let error = verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
    )
    .unwrap_err();
    assert!(error.to_string().contains("prefix 0"));
}

#[test]
fn explicit_account_creation_and_closure_preserve_null_dimensions() {
    for creation in [true, false] {
        let (candidate, mut fixture, mut block, mut output) = context();
        let absent = json!({"presence":"absent", "lamports":null,"owner":null,
            "executable":null,"rentEpoch":null,"dataHash":null,"tokenAmount":null});
        let (state_key, balance_key, token_key) = if creation {
            ("before", "preBalances", "preTokenBalances")
        } else {
            ("after", "postBalances", "postTokenBalances")
        };
        output["original"]["accountTransitions"][0][state_key] = absent.clone();
        block["result"]["transactions"][0]["meta"][balance_key][0] = json!(0);
        block["result"]["transactions"][0]["meta"][token_key] = json!([]);
        if !creation {
            fixture["endAccounts"] =
                json!([{"pubkey":"payer","sourceSlot":20,"presence":"absent"}]);
            output["original"]["endAccountStates"][0]["state"] = absent;
        }
        verify(
            &candidate,
            &fixture,
            &block,
            &output,
            MetadataPolicy::Strict,
        )
        .unwrap();
        output["original"]["accountTransitions"][0][state_key]["lamports"] = json!("0");
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
fn trace_rejects_malformed_bytes_indices_groups_and_unknown_fields() {
    for value in [json!(-1), json!(256), json!(0.5), json!("0"), Value::Null] {
        let (mut trace, archive) = trace_context();
        trace["groups"][0]["instructions"][0]["programIdIndex"] = value;
        assert!(verify_runtime_instruction_trace(&trace, &archive).is_err());
    }
    for value in ["A", "!!!!", "AQ==="] {
        let (mut trace, archive) = trace_context();
        trace["groups"][0]["instructions"][0]["dataBase64"] = json!(value);
        assert!(verify_runtime_instruction_trace(&trace, &archive).is_err());
    }
    for mutation in [
        "duplicate-group",
        "drop",
        "coverage",
        "unknown-field",
        "archive-bytes",
    ] {
        let (mut trace, mut archive) = trace_context();
        match mutation {
            "duplicate-group" => {
                let copy = trace["groups"][0].clone();
                trace["groups"].as_array_mut().unwrap().push(copy);
            }
            "drop" => {
                trace["groups"][0]["instructions"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            "coverage" => trace["coverage"] = json!("all-attempted-calls"),
            "unknown-field" => trace["groups"][0]["instructions"][0]["invented"] = json!(true),
            "archive-bytes" => archive[0]["instructions"][0]["data"] = json!("0"),
            _ => unreachable!(),
        }
        assert!(
            verify_runtime_instruction_trace(&trace, &archive).is_err(),
            "{mutation}"
        );
    }
}
