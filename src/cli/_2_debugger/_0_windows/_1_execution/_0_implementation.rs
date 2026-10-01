use super::*;

pub fn run(
    request: svm_replay_protocol::PreparedRequest,
    config: &svm_replay_engine::Config,
    cancel: svm_replay_engine::shared::runtime::CancellationToken,
) -> Result<Value, Error> {
    use svm_replay_engine::shared::debug::channel;
    use windows_sys::Win32::{
        Foundation::{ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{GetFileType, ReadFile, FILE_TYPE_PIPE},
        System::{
            Console::{GetStdHandle, STD_INPUT_HANDLE},
            Pipes::PeekNamedPipe,
        },
    };
    let stdin = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if stdin.is_null()
        || stdin == INVALID_HANDLE_VALUE
        || unsafe { GetFileType(stdin) } != FILE_TYPE_PIPE
    {
        return Err(Error::new(
            "DEBUG_INPUT",
            "Windows JSONL debugging requires piped stdin",
        ));
    }
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
            .map_err(|e| Error::new("DEBUG_THREAD", e.to_string()))?;
        let mut input = super::super::super::Input::default();
        let mut total = 0usize;
        let mut partial = false;
        let bridged = (|| -> Result<Option<Error>, Error> {
            let mut terminal = None;
            let mut events_done = false;
            loop {
                if cancel.is_cancelled() {
                    return Ok(terminal);
                }
                if Instant::now() >= deadline {
                    return Err(Error::new("DEBUG_DEADLINE", "CLI debug deadline exceeded"));
                }
                let mut drained = true;
                if !events_done {
                    for _ in 0..64 {
                        match controller.try_next_event() {
                            Ok(Some(event)) => {
                                let size = serde_json::to_vec(&event)
                                    .map_err(|e| Error::new("CLI_OUTPUT", e.to_string()))?
                                    .len()
                                    + 1;
                                total = total.checked_add(size).ok_or_else(|| {
                                    Error::new("CLI_OUTPUT_LIMIT", "output accounting overflow")
                                })?;
                                if total > super::super::super::MAX_STREAM {
                                    return Err(Error::new(
                                        "CLI_OUTPUT_LIMIT",
                                        "JSONL stream exceeds output limit",
                                    ));
                                }
                                if let Err(error) = write_final(&event) {
                                    partial = true;
                                    return Err(error);
                                }
                                drained = false;
                            }
                            Ok(None) => {
                                drained = true;
                                break;
                            }
                            Err(error) => {
                                if error.code != "DEBUG_DISCONNECTED" {
                                    terminal = Some(error);
                                }
                                events_done = true;
                                drained = true;
                                break;
                            }
                        }
                    }
                }
                if engine.is_finished() && drained {
                    return Ok(terminal);
                }
                let mut available = 0;
                if unsafe {
                    PeekNamedPipe(
                        stdin,
                        std::ptr::null_mut(),
                        0,
                        std::ptr::null_mut(),
                        &mut available,
                        std::ptr::null_mut(),
                    )
                } == 0
                {
                    let error = io::Error::last_os_error();
                    if matches!(error.raw_os_error(), Some(code) if code == ERROR_BROKEN_PIPE as i32 || code == ERROR_PIPE_NOT_CONNECTED as i32)
                    {
                        cancel.cancel();
                        if !input.line.is_empty() {
                            return Err(Error::new("DEBUG_INPUT", "EOF inside a command line"));
                        }
                        return Ok(terminal);
                    }
                    return Err(Error::new(
                        "CLI_IO",
                        format!("inspect debug input: {error}"),
                    ));
                }
                if available > 0 {
                    let mut bytes = [0u8; 8192];
                    let mut read = 0;
                    let length = available.min(bytes.len() as u32);
                    if unsafe {
                        ReadFile(
                            stdin,
                            bytes.as_mut_ptr(),
                            length,
                            &mut read,
                            std::ptr::null_mut(),
                        )
                    } == 0
                    {
                        return Err(Error::new(
                            "CLI_IO",
                            format!("read debug input: {}", io::Error::last_os_error()),
                        ));
                    }
                    if read == 0 {
                        cancel.cancel();
                        return Ok(terminal);
                    }
                    input.feed(&bytes[..read as usize], &controller)?;
                } else {
                    thread::sleep(Duration::from_millis(5));
                }
            }
        })();
        if !engine.is_finished() {
            cancel.cancel();
        }
        drop(controller);
        let result = engine
            .join()
            .map_err(|_| Error::new("DEBUG_THREAD", "debug engine thread panicked"))?;
        let bridge_error = match bridged {
            Err(error) => Some(error),
            Ok(Some(error)) => {
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
            Ok(None) => None,
        };
        if let Some(error) = bridge_error {
            let mut details = match &result {
                Ok(receipt) => {
                    serde_json::json!({"engineReceiptPath":receipt["receiptPath"], "engineOutcome":receipt["outcome"]})
                }
                Err(error) => serde_json::json!({"engineError":error}),
            };
            details["partialStdoutLine"] = Value::Bool(partial);
            return Err(error.with_details(details));
        }
        result
    })
}
