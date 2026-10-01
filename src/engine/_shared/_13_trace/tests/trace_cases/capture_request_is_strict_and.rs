use super::*;

#[test]
fn slower_hosts_can_request_more_capture_time_with_a_finite_ceiling() {
    let mut value = policy();
    for timeout in [180_000, 600_000, 900_000] {
        value["limits"]["timeoutMs"] = json!(timeout);
        bounds()
            .validate(&request(&value))
            .expect("explicit deadline");
    }
    value["limits"]["timeoutMs"] = json!(900_001);
    assert!(bounds().validate(&request(&value)).is_err());
}

#[test]
fn capture_request_is_strict_and_budgeted_without_coercion() {
    let valid = policy();
    request(&valid).validate().expect("valid policy");
    for field in ["maxBytes", "maxEvents", "timeoutMs"] {
        for invalid in [
            json!(0),
            json!(-1),
            json!(1.5),
            json!("1"),
            Value::Null,
            json!(9_007_199_254_740_992u64),
        ] {
            let mut value = valid.clone();
            value["limits"][field] = invalid;
            assert!(CaptureRequest::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        }
    }
    for (pointer, invalid) in [
        ("/level", json!("auto")),
        ("/executionMode", json!("automatic")),
        ("/filter/programIds", json!(["1".repeat(33)])),
        ("/filter/instructionIndices", json!([1, 1])),
        ("/sbpfObservations", json!("pc-registers")),
    ] {
        let mut value = valid.clone();
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(CaptureRequest::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    let mut extra = valid.clone();
    extra["enabled"] = json!(true);
    assert!(CaptureRequest::parse(&serde_json::to_vec(&extra).unwrap()).is_err());
    assert!(CaptureRequest::parse(br#"{"schema":"one","schema":"two"}"#).is_err());
}

#[test]
fn interpreter_capture_mode_requires_explicit_producer_evidence() {
    let mut value = policy();
    value["executionMode"] = json!("interpreter");
    let request = request(&value);
    let mut producer = output(vec![]);
    assert!(svm_replay_engine::shared::trace::validate_execution_mode(
        &producer,
        &request.execution_mode
    )
    .is_err());
    producer["execution_mode"] = json!("interpreter");
    svm_replay_engine::shared::trace::validate_execution_mode(&producer, &request.execution_mode)
        .expect("explicit native interpreter evidence");
    let mut worker = worker();
    assert_eq!(
        validate_capability(&request, &worker, &worker)
            .unwrap_err()
            .code,
        "CAPABILITY_UNAVAILABLE"
    );
    worker
        .capabilities
        .push("runtime-interpreter-capture/v1".into());
    validate_capability(&request, &worker, &worker).expect("explicit interpreter capability");
}

#[test]
fn capability_and_identity_bind_exact_mode_worker_and_producer_bounds() {
    let policy = request(&policy());
    let worker = worker();
    let input = Digest::of(b"immutable fixture");
    validate_capability(&policy, &worker, &worker).unwrap();
    let identity = capture_identity(&input, &worker, &policy, &bounds()).unwrap();
    let mut reordered = worker.clone();
    reordered.capabilities.reverse();
    assert_eq!(
        identity,
        capture_identity(&input, &reordered, &policy, &bounds()).unwrap()
    );
    let mut missing = worker.clone();
    missing
        .capabilities
        .retain(|cap| cap != "runtime-call-journal/v1");
    assert_eq!(
        validate_capability(&policy, &missing, &missing)
            .unwrap_err()
            .code,
        "CAPABILITY_UNAVAILABLE"
    );
    assert_eq!(
        validate_capability(&policy, &worker, &missing)
            .unwrap_err()
            .code,
        "WORKER_IDENTITY"
    );
    let mut value = serde_json::to_value(&policy).unwrap();
    value["executionMode"] = json!("interpreter-debug");
    assert_ne!(
        identity,
        capture_identity(&input, &worker, &request(&value), &bounds()).unwrap()
    );
    let mut changed = bounds();
    changed.max_invocations += 1;
    assert_ne!(
        identity,
        capture_identity(&input, &worker, &policy, &changed).unwrap()
    );
    changed.register_rows = 1;
    assert!(changed.validate(&policy).is_err());
}

#[test]
fn export_preserves_u64_registers_and_rejects_invalid_executable_rows() {
    let mut policy = policy();
    policy["level"] = json!("sbpf");
    policy["sbpfObservations"] = json!("pc-registers");
    let policy = request(&policy);
    let identity = Digest::of(b"protocol sample");
    let mut value = output(journal());
    value["captures"] = json!([{"ordinal":0,"signatures":[],"execution_mode":"jit","disposition":"COMPLETE","memory":null,
        "invocations":[{"invocation_index":0,"caller_index":65535,"depth":1,"program_id":"11111111111111111111111111111111",
            "elf_sha256":Digest::of(b"explicit synthetic executable"),"text_base64":STANDARD.encode([0x95,0,0,0,0,0,0,0])}],
        "trace":[[0,[u64::MAX,0,0,0,0,0,0,0,0,0,0,0]]]}]);
    let exported = make_export(&policy, &identity, &value).unwrap();
    assert!(String::from_utf8(exported.payload.clone())
        .unwrap()
        .contains("18446744073709551615"));
    assert_eq!(exported.artifact["status"], "COMPLETE");
    validate_artifact(
        &exported.artifact,
        &policy,
        &identity,
        Some(&exported.payload),
    )
    .unwrap();
    for (pointer, bad) in [
        ("/captures/0/trace/0/1/0", json!("18446744073709551615")),
        ("/captures/0/trace/0/1/11", json!(1)),
        ("/captures/0/trace/0/0", json!(1)),
        ("/captures/0/invocations/0/caller_index", json!(0)),
        ("/captures/0/invocations/0/text_base64", json!("!")),
        ("/captures/0/execution_mode", json!("interpreter-debug")),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = bad;
        assert!(
            make_export(&policy, &identity, &changed).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn journal_nesting_raw_accounts_cu_and_duplicate_keys_are_checked() {
    let mut events = journal();
    let key = "11111111111111111111111111111111";
    events.insert(1,json!({"kind":"enter","call_id":0,"parent_id":null,"invocation":{"outer_instruction_index":0,"program_id":key,
        "instruction_data":[1],"accounts":[],"remaining_cu":"100"}}));
    events.insert(2,json!({"kind":"exit","call_id":0,"parent_id":null,"completion":{"remaining_cu":"90","outcome":{"status":"success"},
        "return_data":null,"accounts":[]},"inclusive_cu":"10","exclusive_cu":"10"}));
    let identity = Digest::of(b"journal sample");
    let policy = request(&policy());
    assert_eq!(
        make_export(&policy, &identity, &output(events.clone()))
            .unwrap()
            .artifact["status"],
        "COMPLETE"
    );
    for (index, field, bad) in [
        (2, "inclusive_cu", json!("11")),
        (2, "parent_id", json!(0)),
        (1, "call_id", json!(1)),
        (
            0,
            "accounts",
            json!([{"pubkey":key,"state":{"dataHash":"not raw"}}]),
        ),
    ] {
        let mut changed = events.clone();
        changed[index][field] = bad;
        assert!(make_export(&policy, &identity, &output(changed)).is_err());
    }
    let mut value = output(events);
    value["journals"][0]["journal"]["payload_json"] =
        json!("[{\"kind\":\"transaction_begin\",\"kind\":\"transaction_commit\"}]");
    value["journals"][0]["journal"]["event_count"] = json!(1);
    assert!(make_export(&policy, &identity, &value).is_err());
}
