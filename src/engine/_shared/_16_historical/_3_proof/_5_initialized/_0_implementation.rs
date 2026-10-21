use super::*;

pub(in super::super::super) fn load(
    input: &SessionInput,
    receipt: &Value,
    budget: &ExecutionBudget,
) -> Result<Prepared, Error> {
    check(
        !input.after
            && receipt["schema"] == "svm-replay-receipt/v1"
            && receipt["outcome"] == "COMPLETED"
            && receipt["reconstructedFromSources"] == true
            && receipt["currentStateFallback"] == false
            && receipt["verification"]["status"] == "INITIALIZED"
            && receipt["originalControl"]["status"] == "NOT_APPLICABLE",
        "completed source initialization without a transaction control is required",
    )?;
    let root = input
        .receipt_path
        .parent()
        .ok_or_else(|| invalid("initialization receipt directory missing"))?;
    let packet = parse_json(&_1_artifacts::artifact(
        root,
        &receipt["initializedRequest"],
        256 * 1024 * 1024,
        budget,
    )?)?;
    let request = _7_initialize::validate(&packet)?;
    let block = request.validate()?;
    let fixture = packet["fixture"].clone();
    let result = parse_json(&_1_artifacts::artifact(
        root,
        &receipt["output"],
        256 * 1024 * 1024,
        budget,
    )?)?;
    let binding = &request.runtime_binding;
    let executor = &binding["executor"];
    let worker = &receipt["worker"];
    check(
        worker["family"] == request.family
            && worker["executorSourceId"] == executor["id"]
            && worker["sha256"] == executor["m9Build"]["binarySha256"]
            && worker["buildHash"] == executor["m9Build"]["buildHash"]
            && worker["sourceSha256"] == executor["sourceEvidenceHash"],
        "initialized worker identity differs",
    )?;
    check(
        executor["m9Build"]["capabilities"]
            .as_array()
            .is_some_and(|caps| {
                [
                    "native-initialized-slot/v1",
                    "native-session-socket/v1",
                    "native-transaction-rpc/v1",
                ]
                .iter()
                .all(|required| caps.iter().any(|cap| cap == required))
            }),
        "initialized worker capabilities missing",
    )?;
    check(
        receipt["verification"]["runtimeBindingHash"] == binding["bindingHash"]
            && result["schema"] == "svm-native-initialization-result/v1"
            && result["original"].is_null()
            && result["originalControl"] == packet["originalControl"]
            && result["ready"]["boundaryKind"] == "initialized-slot"
            && result["ready"]["original"].is_null()
            && result["ready"]["state"]["slot"] == request.slot
            && result["ready"]["state"]["transactionIndex"] == 0,
        "initialization result differs from the retained source position",
    )?;
    let block = &block["result"];
    let mut pins = json!({"schema":"svm-historical-session-pins/v1","boundaryKind":"initialized-slot",
        "genesisHash":request.genesis_hash,"signature":null,"slot":request.slot,"transactionIndex":0,
        "after":false,"afterSlot":request.after_slot,"family":request.family,"runtime":receipt["runtime"],
        "blockHeight":integer(&block["blockHeight"])?,"parentSlot":block["parentSlot"],
        "executionBlockhash":block["previousBlockhash"],"blockhash":block["blockhash"],
        "runtimeBindingHash":binding["bindingHash"],"worker":worker,"receiptSha256":input.receipt_sha256,
        "preparedRequestSha256":receipt["initializedRequest"]["sha256"],"originalOutputSha256":null,
        "initializationOutputSha256":receipt["output"]["sha256"],"originalControl":packet["originalControl"],
        "capabilities":executor["m9Build"]["capabilities"]});
    if let Some(selection) = executor.get("runtimeSelection") {
        pins["runtimeSelection"] = selection.clone();
    }
    let rpc_context = _4_context::build(
        block,
        &request.block_sha256,
        &fixture,
        false,
        &pins["capabilities"],
    )?;
    let initialized_state_sha256 = text(&result["ready"]["state"]["stateSha256"])?;
    Digest::new(initialized_state_sha256)?;
    Ok(Prepared {
        worker: WorkerSpec {
            executable: input.worker_path.clone(),
            sha256: text(&worker["sha256"])?.into(),
        },
        fixture,
        original: Value::Null,
        pins,
        rpc_context,
        initialized_state_sha256: Some(initialized_state_sha256.into()),
        boundary: block.clone(),
    })
}
