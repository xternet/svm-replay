use super::*;

#[test]
fn final_json_can_exceed_debug_stream_limit_without_relaxing_debug_stream() {
    let value = json!({"trace":"x".repeat(17 * 1024 * 1024)});
    assert!(Output::default().push(&value).is_err());
    let file = tempfile::tempfile().unwrap();
    write_final_fd(&value, file.as_raw_fd(), Duration::from_secs(5)).unwrap();
    assert!(file.metadata().unwrap().len() > 16 * 1024 * 1024);
}

#[test]
fn fragmented_crlf_commands_are_strict_duplicate_safe_and_bounded() {
    let (controller, _driver) = channel();
    let mut input = Input::default();
    let bytes = command();
    input.feed(&bytes[..10], &controller).unwrap();
    assert_eq!(input.commands, 0);
    input.feed(&bytes[10..], &controller).unwrap();
    input.feed(b"\r\n", &controller).unwrap();
    assert_eq!(input.commands, 1);
    assert!(input.line.is_empty());
    for invalid in [
        b"\n".as_slice(),
        b"{\"requestId\":1,\"requestId\":2}\n",
        b"{}\n",
    ] {
        assert_eq!(
            Input::default()
                .feed(invalid, &controller)
                .unwrap_err()
                .code,
            "DEBUG_INPUT"
        );
    }
    assert_eq!(
        Input::default()
            .feed(&vec![b'x'; MAX_LINE + 1], &controller)
            .unwrap_err()
            .code,
        "DEBUG_INPUT_LIMIT"
    );
}

#[test]
fn completed_engine_events_are_delivered_with_stdin_held_open() {
    let (controller, mut driver) = channel();
    driver
        .complete(&json!({"proof":"test transport only"}))
        .unwrap();
    drop(driver);
    let (input, _held_input) = UnixStream::pair().unwrap();
    let (output, reader) = UnixStream::pair().unwrap();
    let _read = Nonblocking::new(input.as_raw_fd()).unwrap();
    let _write = Nonblocking::new(output.as_raw_fd()).unwrap();
    let cancel = CancellationToken::new();
    let mut queue = Output::default();
    assert!(pump(
        &controller,
        &cancel,
        || true,
        input.as_raw_fd(),
        output.as_raw_fd(),
        Instant::now() + Duration::from_secs(1),
        &mut queue
    )
    .unwrap()
    .is_none());
    reader
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut text = String::new();
    BufReader::new(reader).read_line(&mut text).unwrap();
    let event: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(event["kind"], "session-complete");
    assert!(!cancel.is_cancelled());
    assert_eq!(queue.queued, 0);
}

#[test]
fn eof_and_partial_eof_cancel_without_a_reader_thread() {
    for partial in [false, true] {
        let (controller, _driver) = channel();
        let (input, mut writer) = UnixStream::pair().unwrap();
        let (output, _reader) = UnixStream::pair().unwrap();
        if partial {
            writer.write_all(b"{\"requestId\":").unwrap();
        }
        drop(writer);
        let _read = Nonblocking::new(input.as_raw_fd()).unwrap();
        let _write = Nonblocking::new(output.as_raw_fd()).unwrap();
        let cancel = CancellationToken::new();
        let mut queue = Output::default();
        let result = pump(
            &controller,
            &cancel,
            || false,
            input.as_raw_fd(),
            output.as_raw_fd(),
            Instant::now() + Duration::from_secs(1),
            &mut queue,
        );
        if partial {
            assert_eq!(result.unwrap_err().code, "DEBUG_INPUT");
        } else {
            assert!(result.unwrap().is_none());
        }
        assert!(cancel.is_cancelled());
    }
}

#[test]
fn output_backpressure_does_not_hide_cancellation_or_leave_detached_reader() {
    let (controller, mut driver) = channel();
    driver.complete(&json!({"test":true})).unwrap();
    let (input, _writer) = UnixStream::pair().unwrap();
    let (mut output, _reader) = UnixStream::pair().unwrap();
    fill(&mut output);
    let _read = Nonblocking::new(input.as_raw_fd()).unwrap();
    let cancel = CancellationToken::new();
    let stop = cancel.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        stop.cancel();
    });
    let started = Instant::now();
    let mut queue = Output::default();
    pump(
        &controller,
        &cancel,
        || false,
        input.as_raw_fd(),
        output.as_raw_fd(),
        started + Duration::from_secs(1),
        &mut queue,
    )
    .unwrap();
    cancel_thread.join().unwrap();
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(queue.queued > 0);
}

#[test]
fn final_emitter_is_bounded_and_restores_descriptor_flags() {
    let (output, reader) = UnixStream::pair().unwrap();
    let fd = output.as_raw_fd();
    let before = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    write_final_fd(&json!({"outcome":"COMPLETED"}), fd, Duration::from_secs(1)).unwrap();
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFL) }, before);
    let mut text = String::new();
    BufReader::new(reader).read_line(&mut text).unwrap();
    assert_eq!(text, "{\"outcome\":\"COMPLETED\"}\n");
    let (mut output, _reader) = UnixStream::pair().unwrap();
    fill(&mut output);
    let before = unsafe { libc::fcntl(output.as_raw_fd(), libc::F_GETFL) };
    let error = write_final_fd(
        &json!({"outcome":"ERROR"}),
        output.as_raw_fd(),
        Duration::from_millis(10),
    )
    .unwrap_err();
    assert_eq!(error.code, "CLI_OUTPUT_DEADLINE");
    assert_eq!(
        unsafe { libc::fcntl(output.as_raw_fd(), libc::F_GETFL) },
        before
    );
}

#[test]
fn terminal_engine_failure_is_retained_for_joined_receipt_validation() {
    let (controller, mut driver) = channel();
    driver
        .failed(&Error::new("DEBUG_TEST", "explicit fixture failure"))
        .unwrap();
    let (input, _writer) = UnixStream::pair().unwrap();
    let (output, _reader) = UnixStream::pair().unwrap();
    let _read = Nonblocking::new(input.as_raw_fd()).unwrap();
    let _write = Nonblocking::new(output.as_raw_fd()).unwrap();
    let mut queue = Output::default();
    let terminal = pump(
        &controller,
        &CancellationToken::new(),
        || true,
        input.as_raw_fd(),
        output.as_raw_fd(),
        Instant::now() + Duration::from_secs(1),
        &mut queue,
    )
    .unwrap()
    .unwrap();
    assert_eq!(terminal.code, "DEBUG_TEST");
}
