use super::*;

pub(in super::super) fn pump(
    controller: &DebugController,
    cancel: &CancellationToken,
    finished: impl Fn() -> bool,
    input_fd: i32,
    output_fd: i32,
    deadline: Instant,
    output: &mut Output,
) -> Result<Option<Error>, Error> {
    let mut input = Input::default();
    let mut events_done = false;
    let mut terminal = None;
    loop {
        if cancel.is_cancelled() {
            return Ok(terminal);
        }
        if Instant::now() >= deadline {
            return Err(Error::new("DEBUG_DEADLINE", "CLI debug deadline exceeded"));
        }
        if !events_done {
            for _ in 0..64 {
                match controller.try_next_event() {
                    Ok(Some(event)) => output.push(&event)?,
                    Ok(None) => break,
                    Err(error) => {
                        if error.code != "DEBUG_DISCONNECTED" {
                            terminal = Some(error);
                        }
                        events_done = true;
                        break;
                    }
                }
            }
        }
        output.flush(output_fd)?;
        if finished() && output.queued == 0 {
            // Drain queued events even when the engine has already returned.
            if events_done {
                return Ok(terminal);
            }
            match controller.try_next_event() {
                Ok(Some(event)) => {
                    output.push(&event)?;
                    continue;
                }
                Ok(None) => return Ok(terminal),
                Err(error) if error.code == "DEBUG_DISCONNECTED" => return Ok(terminal),
                Err(error) => return Ok(Some(error)),
            }
        }
        let mut fds = [
            libc::pollfd {
                fd: input_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: output_fd,
                events: if output.queued > 0 { libc::POLLOUT } else { 0 },
                revents: 0,
            },
        ];
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 5) };
        if ready < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(io_error("poll debug descriptors"));
        }
        if fds.iter().any(|fd| fd.revents & libc::POLLNVAL != 0) {
            return Err(Error::new("CLI_IO", "invalid debug descriptor"));
        }
        if fds[1].revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            return Err(Error::new("CLI_IO", "debug output consumer disconnected"));
        }
        if fds[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            let mut bytes = [0u8; 8192];
            let count = unsafe { libc::read(input_fd, bytes.as_mut_ptr().cast(), bytes.len()) };
            if count == 0 {
                cancel.cancel();
                if !input.line.is_empty() {
                    return Err(Error::new("DEBUG_INPUT", "EOF inside a command line"));
                }
                return Ok(terminal);
            }
            if count < 0 {
                match io::Error::last_os_error().kind() {
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted => continue,
                    _ => return Err(io_error("read debug command")),
                }
            }
            input.feed(&bytes[..count as usize], controller)?;
        }
    }
}

pub fn run(
    request: PreparedRequest,
    config: &Config,
    cancel: CancellationToken,
) -> Result<Value, Error> {
    let _input = Nonblocking::new(libc::STDIN_FILENO)?;
    let _output = Nonblocking::new(libc::STDOUT_FILENO)?;
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(request.limits.timeout_ms))
        .ok_or_else(|| Error::new("DEBUG_CONFIG", "invalid CLI debug deadline"))?;
    let (controller, mut driver) = channel();
    thread::scope(|scope| {
        let engine_cancel = cancel.clone();
        let engine = thread::Builder::new()
            .name("svm-debug".into())
            .spawn_scoped(scope, move || {
                svm_replay_engine::debug_prepared(request, config, engine_cancel, &mut driver)
            })
            .map_err(|error| Error::new("DEBUG_THREAD", error.to_string()))?;
        let mut output = Output::default();
        let bridged = pump(
            &controller,
            &cancel,
            || engine.is_finished(),
            libc::STDIN_FILENO,
            libc::STDOUT_FILENO,
            deadline,
            &mut output,
        );
        if !engine.is_finished() {
            cancel.cancel();
        }
        drop(controller);
        let result = match engine.join() {
            Ok(result) => result,
            Err(_) => Err(Error::new("DEBUG_THREAD", "debug engine thread panicked")),
        };
        // Complete any partly written JSONL frame before the root emits its
        // final receipt. Children are already joined even if output is blocked.
        let flushed = flush_bounded(&mut output, libc::STDOUT_FILENO, Duration::from_secs(5));
        let bridge_error = match (bridged, flushed) {
            (_, Err(error)) | (Err(error), Ok(())) => Some(error),
            (Ok(Some(error)), Ok(())) => {
                let recorded = match &result {
                    Ok(receipt) => receipt["error"]["code"] == error.code,
                    Err(engine) => engine.code == error.code,
                };
                if recorded {
                    None
                } else {
                    Some(error)
                }
            }
            (Ok(None), Ok(())) => None,
        };
        if let Some(error) = bridge_error {
            let mut details = match &result {
                Ok(receipt) => {
                    serde_json::json!({"engineReceiptPath":receipt["receiptPath"],"engineOutcome":receipt["outcome"]})
                }
                Err(error) => serde_json::json!({"engineError":error}),
            };
            details["partialStdoutLine"] = Value::Bool(output.offset != 0);
            return Err(error.with_details(details));
        }
        result
    })
}

/// Root CLI emitter, also safe when a consumer leaves its output pipe full.
pub(crate) fn write_final(value: &Value) -> Result<(), Error> {
    if value.pointer("/error/details/partialStdoutLine") == Some(&Value::Bool(true)) {
        return Err(Error::new(
            "CLI_OUTPUT",
            "cannot append final receipt to an incomplete stdout frame",
        ));
    }
    write_final_fd(value, libc::STDOUT_FILENO, Duration::from_secs(5))
}
pub(in super::super) fn write_final_fd(
    value: &Value,
    fd: i32,
    timeout: Duration,
) -> Result<(), Error> {
    let _mode = Nonblocking::new(fd)?;
    let mut output = Output::default();
    output.push_bounded(value, 512 * 1024 * 1024)?;
    flush_bounded(&mut output, fd, timeout)
}
pub(in super::super) fn flush_bounded(
    output: &mut Output,
    fd: i32,
    timeout: Duration,
) -> Result<(), Error> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| Error::new("CLI_OUTPUT_LIMIT", "invalid output deadline"))?;
    loop {
        output.flush(fd)?;
        if output.queued == 0 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::new(
                "CLI_OUTPUT_DEADLINE",
                "stdout consumer did not accept final receipt",
            ));
        }
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        };
        if unsafe { libc::poll(&mut poll, 1, 5) } < 0
            && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
        {
            return Err(io_error("poll final receipt output"));
        }
    }
}
