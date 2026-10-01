use super::*;

#[test]
fn register_volume_does_not_starve_requested_memory_rows() {
    let mut policy = policy();
    policy["level"] = json!("sbpf");
    policy["sbpfObservations"] = json!("pc-registers-memory");
    policy["limits"]["maxEvents"] = json!(8);
    let mut value = output(journal());
    let registers = vec![json!([0, vec![0; 12]]); 20];
    value["captures"] = json!([{"ordinal":0,"signatures":[],"execution_mode":"jit","disposition":"COMPLETE",
        "trace":registers,"invocations":[{"invocation_index":0,"caller_index":65535,"depth":1,
        "program_id":"11111111111111111111111111111111","elf_sha256":Digest::of(b"test ELF"),"text_base64":STANDARD.encode([0;8])}],
        "memory":{"scope":"SBPF loads/stores only","disposition":"COMPLETE","rows":[{"invocation_index":0,"pc":"0x0","address":"0x1000","width":8,
            "operation":"load","succeeded":false,"value":null,"before":null,"after":null}]}}]);
    let exported = make_export(&request(&policy), &Digest::of(b"fair export"), &value).unwrap();
    let events = svm_replay_protocol::parse_json(&exported.payload).unwrap();
    for kind in ["registers", "memory"] {
        assert!(
            events
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["kind"] == kind),
            "{kind}"
        );
    }
    assert_eq!(exported.artifact["status"], "TRUNCATED");
}

#[test]
fn wide_register_values_are_exported_losslessly_for_javascript() {
    let mut policy = policy();
    policy["level"] = json!("sbpf");
    policy["sbpfObservations"] = json!("pc-registers");
    let mut value = output(journal());
    let mut registers = vec![0_u64; 12];
    registers[0] = u64::MAX;
    value["captures"] = json!([{"ordinal":0,"signatures":[],"execution_mode":"jit","disposition":"COMPLETE",
        "trace":[[0,registers]],"invocations":[{"invocation_index":0,"caller_index":65535,"depth":1,
        "program_id":"11111111111111111111111111111111","elf_sha256":Digest::of(b"test ELF"),"text_base64":STANDARD.encode([0;8])}]}]);
    let export = make_export(&request(&policy), &Digest::of(b"wide registers"), &value).unwrap();
    let events = svm_replay_protocol::parse_json(&export.payload).unwrap();
    let event = events
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "registers")
        .unwrap();
    assert_eq!(event["row"][1][0], u64::MAX.to_string());
    assert_eq!(event["row"][1][1], 0);
}

#[test]
fn truncation_partial_memory_and_payload_integrity_are_explicit() {
    let identity = Digest::of(b"bounded export");
    let mut value = output(journal());
    let mut policy = policy();
    policy["limits"]["maxEvents"] = json!(1);
    let exported = make_export(&request(&policy), &identity, &value).unwrap();
    assert_eq!(exported.artifact["status"], "TRUNCATED");
    assert_eq!(exported.artifact["payload"]["events"], 1);
    policy["limits"]["maxBytes"] = json!(2);
    assert_eq!(
        make_export(&request(&policy), &identity, &value)
            .unwrap()
            .payload,
        b"[]"
    );
    policy["limits"]["maxBytes"] = json!(1);
    assert!(make_export(&request(&policy), &identity, &value).is_err());
    let policy = request(&super_policy());
    value["journals"][0]["journal"]["disposition"] = json!("PARTIAL");
    value["journals"][0]["journal"]["reason"] = json!("observed partial journal");
    let exported = make_export(&policy, &identity, &value).unwrap();
    assert_eq!(exported.artifact["status"], "PARTIAL");
    assert!(validate_artifact(&exported.artifact, &policy, &identity, Some(b"[]")).is_err());
    let mut lied = exported.artifact.clone();
    lied["reason"] = Value::Null;
    assert!(validate_artifact(&lied, &policy, &identity, Some(&exported.payload)).is_err());
}

#[test]
fn requested_memory_keeps_faults_and_rejects_invalid_raw_rows() {
    let mut policy = policy();
    policy["level"] = json!("sbpf");
    policy["sbpfObservations"] = json!("pc-registers-memory");
    let policy = request(&policy);
    let identity = Digest::of(b"memory protocol");
    let mut value = output(journal());
    value["captures"] = json!([{"ordinal":0,"signatures":[],"execution_mode":"jit","disposition":"COMPLETE","trace":[],"invocations":[],
        "memory":{"scope":"SBPF loads/stores only","disposition":"COMPLETE","rows":[{"invocation_index":0,"pc":"0x0","address":"0x1000","width":8,
            "operation":"load","succeeded":false,"value":null,"before":null,"after":null}]}}]);
    let exported = make_export(&policy, &identity, &value).unwrap();
    assert_eq!(exported.artifact["status"], "COMPLETE");
    let events = svm_replay_protocol::parse_json(&exported.payload).unwrap();
    assert!(events
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["kind"] == "memory" && event["row"]["succeeded"] == false));
    for (field, bad) in [
        ("width", json!(3)),
        ("operation", json!("host-copy")),
        ("address", json!("overflow")),
        ("value", json!([256, 0, 0, 0, 0, 0, 0, 0])),
        ("succeeded", json!(true)),
    ] {
        let mut changed = value.clone();
        changed["captures"][0]["memory"]["rows"][0][field] = bad;
        assert!(
            make_export(&policy, &identity, &changed).is_err(),
            "{field}"
        );
    }
    value["captures"][0]["memory"] = Value::Null;
    assert!(make_export(&policy, &identity, &value).is_err());
}

#[test]
fn newest_runtime_load_snapshots_must_equal_the_observed_value() {
    let mut policy = policy();
    policy["level"] = json!("sbpf");
    policy["sbpfObservations"] = json!("pc-registers-memory");
    let mut value = output(journal());
    let bytes = json!([6, 0, 0, 0, 0, 0, 0, 0]);
    value["captures"] = json!([{"ordinal":0,"signatures":[],"execution_mode":"jit","disposition":"COMPLETE","trace":[],"invocations":[],
        "memory":{"scope":"SBPF loads/stores only","disposition":"COMPLETE","rows":[{"invocation_index":0,"pc":"0x0","address":"0x1000","width":8,
            "operation":"load","succeeded":true,"value":bytes,"before":bytes,"after":bytes}]}}]);
    let identity = Digest::of(b"load snapshots");
    assert!(make_export(&request(&policy), &identity, &value).is_ok());
    value["captures"][0]["memory"]["rows"][0]["after"][0] = json!(7);
    assert!(make_export(&request(&policy), &identity, &value).is_err());
}

#[test]
fn calls_cannot_be_relabelled_from_interpreter_as_jit() {
    let mut value = output(journal());
    value["execution_mode"] = json!("interpreter-debug");
    assert!(make_export(&request(&policy()), &Digest::of(b"mode"), &value).is_err());
}

#[test]
fn per_execution_mode_is_required_when_the_producer_omits_top_level_mode() {
    let mut value = output(journal());
    value.as_object_mut().unwrap().remove("execution_mode");
    // The reviewed newest producer records mode on each captured execution,
    // including call-only exports. No missing field is filled from the request.
    value["captures"] = json!([{"execution_mode":"jit"}]);
    assert!(make_export(&request(&policy()), &Digest::of(b"mode"), &value).is_ok());
    for captures in [
        json!([]),
        json!([{}]),
        json!([{"execution_mode":null}]),
        json!([{"execution_mode":"interpreter-debug"}]),
        json!([{"execution_mode":"jit"},{"execution_mode":"interpreter-debug"}]),
    ] {
        value["captures"] = captures;
        assert!(make_export(&request(&policy()), &Digest::of(b"mode"), &value).is_err());
    }
    value["captures"] = json!([{"execution_mode":"jit"}]);
    value["execution_mode"] = Value::Null;
    assert!(make_export(&request(&policy()), &Digest::of(b"mode"), &value).is_err());
    value["execution_mode"] = json!("jit");
    value["captures"] = json!([{"execution_mode":"interpreter-debug"}]);
    assert!(make_export(&request(&policy()), &Digest::of(b"mode"), &value).is_err());
}

#[test]
fn raw_account_snapshot_requires_actual_canonical_bytes_and_digest() {
    use svm_replay_engine::shared::trace::validate_raw_account_data;
    let bytes = [0u8, 255, 3];
    let value = json!({"dataBase64":STANDARD.encode(bytes),"dataSha256":Digest::of(bytes)});
    validate_raw_account_data(&value).unwrap();
    let mut changed = value.clone();
    changed["dataSha256"] = json!(Digest::of(b"different"));
    assert!(validate_raw_account_data(&changed).is_err());
    changed = value.clone();
    changed["dataBase64"] = json!("!");
    assert!(validate_raw_account_data(&changed).is_err());
    assert!(validate_raw_account_data(&json!({"dataHash":Digest::of(bytes)})).is_err());
}
