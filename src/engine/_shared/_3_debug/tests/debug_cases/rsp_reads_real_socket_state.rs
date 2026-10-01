use super::*;

#[test]
fn rsp_reads_real_socket_state_and_advances_only_on_target_stop() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback");
    let port = listener.local_addr().expect("address").port();
    let target = thread::spawn(move || {
        let (mut stream, peer) = listener.accept().expect("attach");
        assert!(peer.ip().is_loopback());
        let mut registers = [0_u64; 12];
        registers[11] = 0x100000008;
        assert_eq!(receive(&mut stream), "g");
        respond(
            &mut stream,
            &registers
                .iter()
                .flat_map(|r| r.to_le_bytes())
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        );
        assert_eq!(receive(&mut stream), "s");
        registers[11] += 8;
        respond(&mut stream, "S05");
        assert_eq!(receive(&mut stream), "g");
        respond(
            &mut stream,
            &registers
                .iter()
                .flat_map(|r| r.to_le_bytes())
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        );
        assert_eq!(receive(&mut stream), "c");
        respond(&mut stream, "W00");
    });
    let mut client = DebugClient::connect(port, 32768, &budget()).expect("connect");
    assert_eq!(client.registers().expect("registers")[11], 0x100000008);
    client.step().expect("start step");
    assert_eq!(client.state(), DebugState::Running);
    assert!(client.registers().is_err());
    assert_eq!(
        client.wait_stop().expect("step response"),
        Stop::Stopped { signal: 5 }
    );
    assert_eq!(
        client.registers().expect("stepped registers")[11],
        0x100000010
    );
    client.resume().expect("continue");
    assert_eq!(
        client.wait_stop().expect("exit"),
        Stop::VmExited { code: 0 }
    );
    assert_eq!(client.state(), DebugState::VmExited);
    target.join().expect("target");
}

#[test]
fn malformed_and_oversized_rsp_fail_closed() {
    for response in ["$S05#00".to_string(), format!("${}#00", "a".repeat(100))] {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback");
        let port = listener.local_addr().expect("address").port();
        let target = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("attach");
            assert_eq!(receive(&mut stream), "s");
            stream.write_all(response.as_bytes()).expect("bad packet");
        });
        let mut client = DebugClient::connect(port, 64, &budget()).expect("connect");
        client.step().expect("step");
        let error = client.wait_stop().expect_err("bad packet rejected");
        assert!(matches!(
            error.code.as_str(),
            "DEBUG_PROTOCOL" | "DEBUG_LIMIT"
        ));
        assert_eq!(client.state(), DebugState::Error);
        target.join().expect("target");
    }
}

#[test]
fn invocation_metadata_requires_actual_consistent_ancestry_and_elf() {
    let root=format!("execution_mode=interpreter-debug;execution_index=0;program_id=11111111111111111111111111111111;invocation_index=0;cpi_level=0;caller_index=none;caller=none;ancestors=none;elf_sha256={}","a".repeat(64));
    let metadata = InvocationMetadata::parse(&root).expect("runtime identity");
    assert_eq!(metadata.identity.depth, 1);
    for poison in [
        format!("{root};execution_index=1"),
        root.replace("cpi_level=0", "cpi_level=1"),
        root.replace("interpreter-debug", "jit"),
        root.replace("elf_sha256=", "missing="),
    ] {
        assert!(InvocationMetadata::parse(&poison).is_err(), "{poison}");
    }
    // Register truncation may omit invocation rows, but the independent complete
    // live-debug metadata must still match every attached runtime invocation.
    use svm_replay_engine::shared::debug::validate_finalized_invocations;
    let captured =
        serde_json::json!({"captures":[{"ordinal":0,"invocations":[],"debug_invocations":[root]}]});
    assert_eq!(
        validate_finalized_invocations(&captured, &[metadata.clone()], 64)
            .expect("finalized live identity"),
        vec![0]
    );
    let mut poisoned = captured.clone();
    poisoned["captures"][0]["debug_invocations"][0] =
        serde_json::json!(root.replace(&"a".repeat(64), &"b".repeat(64)));
    assert!(validate_finalized_invocations(&poisoned, &[metadata.clone()], 64).is_err());
    poisoned = captured.clone();
    poisoned["captures"][0]["debug_invocations"] = serde_json::json!([]);
    assert!(validate_finalized_invocations(&poisoned, &[metadata], 64).is_err());
}

#[test]
fn qualified_v42_inventory_is_explicitly_ordinal_only_and_checks_every_recorded_identity() {
    use svm_replay_engine::shared::debug::{
        validate_finalized_inventory, FinalizedInventoryPolicy,
    };
    let text=format!("execution_mode=interpreter-debug;execution_index=0;program_id=11111111111111111111111111111111;invocation_index=0;cpi_level=0;caller_index=none;caller=none;ancestors=none;elf_sha256={}","a".repeat(64));
    let metadata = InvocationMetadata::parse(&text).unwrap();
    let capture = serde_json::json!({"captures":[{"ordinal":0,"execution_mode":"interpreter-debug","disposition":"TRUNCATED","invocations":[]}]});
    let policy = FinalizedInventoryPolicy::V42OrdinalAndCapturedIntersection;
    assert!(validate_finalized_inventory(
        &capture,
        &[metadata.clone()],
        64,
        FinalizedInventoryPolicy::Complete
    )
    .is_err());
    let result = validate_finalized_inventory(&capture, &[metadata.clone()], 64, policy).unwrap();
    assert_eq!(result.execution_indices, vec![0]);
    assert_eq!(result.assurance,"live-rsp-identities; finalized-execution-ordinals-and-captured-intersection-only; no-complete-finalized-invocation-inventory");
    assert_eq!(result.finalized_invocations, None);
    let mut record = capture.clone();
    record["captures"][0]["invocations"] = serde_json::json!([{"invocation_index":0,"caller_index":65535,"depth":1,"program_id":"11111111111111111111111111111111","elf_sha256":"a".repeat(64)}]);
    assert_eq!(
        validate_finalized_inventory(&record, &[metadata.clone()], 64, policy)
            .unwrap()
            .captured_intersections,
        1
    );
    for (key, value) in [
        ("invocation_index", serde_json::json!(1)),
        ("caller_index", serde_json::json!(0)),
        ("depth", serde_json::json!(2)),
        ("elf_sha256", serde_json::json!("b".repeat(64))),
    ] {
        let mut poison = record.clone();
        poison["captures"][0]["invocations"][0][key] = value;
        assert!(
            validate_finalized_inventory(&poison, &[metadata.clone()], 64, policy).is_err(),
            "{key}"
        );
    }
    let mut poison = capture.clone();
    poison["captures"][0]["ordinal"] = serde_json::json!(1);
    assert!(validate_finalized_inventory(&poison, &[metadata.clone()], 64, policy).is_err());
    poison = capture.clone();
    poison["captures"][0]["execution_mode"] = serde_json::json!("jit");
    assert!(validate_finalized_inventory(&poison, &[metadata.clone()], 64, policy).is_err());
    poison = capture;
    poison["captures"][0]["debug_invocations"] = serde_json::json!([]);
    assert!(
        validate_finalized_inventory(&poison, &[metadata], 64, policy).is_err(),
        "a present complete inventory cannot be discarded as an ordinal-only fallback"
    );
}
