use super::*;

#[test]
fn delayed_interrupt_does_not_hide_the_first_real_breakpoint_stop() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback");
    let port = listener.local_addr().expect("address").port();
    let target = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("attach");
        assert_eq!(receive(&mut stream), "c");
        let mut interrupt = [0];
        stream.read_exact(&mut interrupt).expect("interrupt");
        assert_eq!(interrupt, [3]);
        respond(&mut stream, "S05");
        assert_eq!(receive(&mut stream), "c");
        respond(&mut stream, "S02");
        assert_eq!(receive(&mut stream), "c");
        respond(&mut stream, "W00");
    });
    let mut client = DebugClient::connect(port, 32768, &budget()).expect("connect");
    client.resume().expect("start");
    client.pause().expect("pause request");
    assert_eq!(
        client.wait_stop().expect("breakpoint wins"),
        Stop::Stopped { signal: 5 }
    );
    client
        .resume()
        .expect("explicit continue drains only outstanding interrupt");
    assert_eq!(
        client.wait_stop().expect("real exit after interrupt drain"),
        Stop::VmExited { code: 0 }
    );
    target.join().expect("target");
}

#[test]
fn account_reads_validate_live_serialized_bytes_without_resuming() {
    use svm_replay_engine::shared::debug::inspect_account;
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback");
    let port = listener.local_addr().expect("address").port();
    let target = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("attach");
        assert_eq!(
            receive(&mut stream),
            format!(
                "qRcmd,{}",
                b"metadata"
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            )
        );
        let metadata=format!("execution_mode=interpreter-debug;execution_index=0;program_id=11111111111111111111111111111111;invocation_index=0;cpi_level=0;caller_index=none;caller=none;ancestors=none;elf_sha256={};account_layout=serialized-addresses-v1;account_count=1;accounts=[[0,\"11111111111111111111111111111111\",\"0x10\",\"0x30\",\"0x50\",\"0x58\",\"0x60\",4,4]]","a".repeat(64));
        respond(
            &mut stream,
            &format!(
                "O{}",
                metadata
                    .bytes()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            ),
        );
        respond(&mut stream, "OK");
        for (command, response) in [
            ("g".to_owned(), "00".repeat(96)),
            ("m10,20".into(), "00".repeat(32)),
            ("m30,20".into(), "01".repeat(32)),
            (
                "m50,8".into(),
                42_u64
                    .to_le_bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
            ),
            (
                "m58,8".into(),
                4_u64
                    .to_le_bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
            ),
            ("m60,2".into(), "abff".into()),
            ("g".into(), "00".repeat(96)),
        ] {
            assert_eq!(receive(&mut stream), command);
            respond(&mut stream, &response);
        }
    });
    let mut client = DebugClient::connect(port, 32768, &budget()).expect("connect");
    let account = inspect_account(&mut client, "11111111111111111111111111111111", 2)
        .expect("paused account");
    assert_eq!(account["lamports"], "42");
    assert_eq!(account["dataHex"], "abff");
    assert_eq!(account["dataLength"], 4);
    assert_eq!(account["disposition"], "TRUNCATED");
    assert_eq!(client.state(), DebugState::Stopped);
    target.join().expect("target");
}

#[test]
fn controller_receives_terminal_failure_without_waiting_for_session_deadline() {
    let (controller, mut driver) = svm_replay_engine::shared::debug::channel();
    assert!(controller
        .try_next_event()
        .expect("nonblocking empty poll")
        .is_none());
    driver
        .failed(&svm_replay_protocol::Error::new(
            "DEBUG_PARITY_MISMATCH",
            "explicit terminal failure",
        ))
        .expect("failure recorded");
    let error = controller
        .next_event(&budget())
        .expect_err("immediate terminal state");
    assert_eq!(error.code, "DEBUG_PARITY_MISMATCH");
}

#[test]
fn rejected_read_remains_explicit_and_does_not_falsely_resume_or_disconnect_vm() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback");
    let port = listener.local_addr().unwrap().port();
    let target = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        assert_eq!(receive(&mut stream), "mffffffff,1");
        respond(&mut stream, "E01");
        assert_eq!(receive(&mut stream), "g");
        respond(&mut stream, &"00".repeat(96));
    });
    let mut client = DebugClient::connect(port, 32768, &budget()).unwrap();
    let error = client
        .memory(u32::MAX as u64, 1)
        .expect_err("unreadable memory");
    assert_eq!(error.code, "DEBUG_TARGET");
    assert_eq!(client.state(), DebugState::Stopped);
    assert_eq!(client.registers().unwrap(), [0; 12]);
    target.join().unwrap();
}
