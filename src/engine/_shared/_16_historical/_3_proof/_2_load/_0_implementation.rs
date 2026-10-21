use super::*;

pub(in super::super::super) fn load(
    input: &SessionInput,
    budget: &ExecutionBudget,
) -> Result<Prepared, Error> {
    check(
        input.schema == "svm-historical-session-input/v1",
        "invalid session input schema",
    )?;
    check(
        input.receipt_path.is_absolute() && input.worker_path.is_absolute(),
        "session paths must be absolute",
    )?;
    let raw = _1_artifacts::read(
        &input.receipt_path,
        &input.receipt_sha256,
        16 * 1024 * 1024,
        budget,
    )?;
    let receipt = parse_json(&raw)?;
    if receipt["scope"] == "source-reconstructed-slot-initialization/v1" {
        return _5_initialized::load(input, &receipt, budget);
    }
    check(
        receipt["schema"] == "svm-replay-receipt/v1"
            && receipt["outcome"] == "COMPLETED"
            && receipt["scope"] == "source-reconstructed-replay/v1"
            && receipt["currentStateFallback"] == false,
        "a completed source-reconstructed receipt without fallback is required",
    )?;
    let root = input
        .receipt_path
        .parent()
        .ok_or_else(|| invalid("receipt directory missing"))?;
    let raw = _1_artifacts::artifact(root, &receipt["preparedRequest"], 256 * 1024 * 1024, budget)?;
    let request = PreparedRequest::parse(&raw)?;
    let block = request.archived_block()?;
    let block = &block["result"];
    let height = integer(&block["blockHeight"])?;
    let result = parse_json(&_1_artifacts::artifact(
        root,
        &receipt["output"],
        256 * 1024 * 1024,
        budget,
    )?)?;
    let source = parse_json(&_1_artifacts::artifact(
        root,
        &receipt["sourcePreparation"],
        256 * 1024 * 1024,
        budget,
    )?)?;
    let fixture = request.fixture;
    let target = &fixture["target"];
    let binding = &fixture["runtime"]["binding"];
    svm_replay_protocol::runtime::validate_runtime_binding(binding)?;
    crate::shared::bank::context::assert_target_blockhash(
        &fixture["runtime"]["bankContext"],
        text(&block["blockhash"])?,
    )?;
    check(
        fixture["runtime"]["bankContext"]["executionBlockhash"] == block["previousBlockhash"],
        "session block lineage differs",
    )?;
    for field in ["whatIf", "accountOverride", "requestedAccountOverrides"] {
        check(
            fixture.get(field).is_none_or(Value::is_null),
            "session requires unmodified original-control inputs",
        )?;
    }
    check(
        target
            .get("replacementTransactionBase64")
            .is_none_or(Value::is_null),
        "replacement is not an original-control input",
    )?;
    for report in [
        &receipt["verification"],
        &receipt["controlVerification"],
        &receipt["controlEvidence"]["verification"],
    ] {
        check(
            report["status"] == "PASS" && report["runtimeBindingHash"] == binding["bindingHash"],
            "verified original-control binding missing or inconsistent",
        )?;
    }
    check(
        receipt["controlEvidence"]["complete"] == true
            && receipt["controlEvidence"]["phase"] == "original-control"
            && receipt["controlEvidence"]["outputSha256"] == receipt["output"]["sha256"]
            && text(&receipt["controlEvidence"]["fixtureSha256"])?
                == Digest::of(&serde_json::to_vec(&fixture).map_err(|e| invalid(e.to_string()))?)
                    .as_str(),
        "original-control artifacts do not bind the prepared fixture and result",
    )?;
    let executor = &binding["executor"];
    let worker = &receipt["worker"];
    check(
        worker["family"] == request.family
            && worker["executorSourceId"] == executor["id"]
            && worker["sha256"] == executor["m9Build"]["binarySha256"]
            && worker["buildHash"] == executor["m9Build"]["buildHash"]
            && worker["sourceSha256"] == executor["sourceEvidenceHash"],
        "worker identity differs from original control",
    )?;
    check(
        executor["m9Build"]["capabilities"]
            .as_array()
            .is_some_and(|caps| {
                ["native-session-socket/v1", "native-transaction-rpc/v1"]
                    .iter()
                    .all(|required| caps.iter().any(|cap| cap == required))
            }),
        "verified worker lacks historical socket/transaction RPC capabilities",
    )?;
    check(
        result["schema"] == "svm-simulate-m6-executor-output/v1"
            && result["runtimeBindingHash"] == binding["bindingHash"]
            && result["targetSignature"] == target["signature"]
            && result["targetIndex"] == target["index"]
            && result["original"].is_object(),
        "control result target/binding differs",
    )?;
    let slot = integer(&target["targetSlot"])?;
    let index = integer(&target["index"])?;
    check(
        source["schema"] == "svm-replay-source-preparation/v1"
            && source["status"] == "PREPARED"
            && source["currentStateFallback"] == false
            && source["slot"] == slot
            && source["transactionIndex"] == index,
        "source preparation does not bind this historical position",
    )?;
    crate::shared::history::address(text(&source["genesisHash"])?, 32)?;
    check(
        !text(&receipt["runtime"]["agave"])?.is_empty(),
        "historical core version missing",
    )?;
    let next = index
        .checked_add(u64::from(input.after))
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| invalid("session position overflow"))?;
    if let Some(after) = fixture["runtime"].get("afterSlot") {
        let after = integer(after)?;
        check(
            !input.after && index == 0 && integer(&block["parentSlot"])? <= after && after < slot,
            "slot boundary does not bind the next produced block before transaction zero",
        )?;
    }
    let mut pins = json!({"schema":"svm-historical-session-pins/v1","genesisHash":source["genesisHash"],
        "signature":target["signature"],"slot":slot,"transactionIndex":next,"after":input.after,"afterSlot":fixture["runtime"]["afterSlot"],
        "family":request.family,"runtime":receipt["runtime"],"blockHeight":height,"parentSlot":block["parentSlot"],
        "executionBlockhash":block["previousBlockhash"],"blockhash":block["blockhash"],
        "runtimeBindingHash":binding["bindingHash"],"worker":worker,
        "receiptSha256":input.receipt_sha256,"preparedRequestSha256":receipt["preparedRequest"]["sha256"],
        "originalOutputSha256":receipt["output"]["sha256"],"sourceEvidenceHashes":request.source_evidence_hashes,
        "capabilities":executor["m9Build"]["capabilities"]});
    if let Some(selection) = executor.get("runtimeSelection") {
        pins["runtimeSelection"] = selection.clone();
    }
    let rpc_context = _4_context::build(
        block,
        &request.block_sha256,
        &fixture,
        input.after,
        &pins["capabilities"],
    )?;
    Ok(Prepared {
        worker: WorkerSpec {
            executable: input.worker_path.clone(),
            sha256: text(&worker["sha256"])?.into(),
        },
        fixture,
        original: result["original"].clone(),
        pins,
        rpc_context,
        initialized_state_sha256: None,
        boundary: block.clone(),
    })
}
