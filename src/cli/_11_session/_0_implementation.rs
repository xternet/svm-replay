use super::*;

pub(crate) fn run(options: Options) -> Result<Value, Error> {
    // A store and its journal must agree, and a journal must belong to this `--after`, before
    // anything is prepared or created.
    let selector = options.after.clone();
    let recorded = match &options.journal {
        Some(path) => {
            require_journal(path, options.journal_require.as_deref())?;
            recorded_journal_input(path, selector.as_deref())?
        }
        None => None,
    };
    // Reserve the endpoints first: an occupied port cannot initiate preparation.
    let endpoints = match options.backend {
        Some(_) => None,
        None => Some(listeners(options.port)?),
    };
    let cancellation = crate::lifecycle::cancellation::install()?;
    bind_parent(options.parent_pid)?;
    // An embedded backend lives as long as the tool that owns it (it is bound to that
    // parent); the public server keeps its one-hour default.
    let timeout = match (options.controls.timeout, &options.backend) {
        (Some(timeout), _) => timeout,
        (None, Some(_)) => 30 * 24 * 3600,
        (None, None) => 3600,
    };
    let budget = ExecutionBudget::new(Duration::from_secs(timeout), cancellation.clone())
        .map_err(protocol_error)?;
    let data_dir = crate::arguments::data_directory(options.data_dir)?;
    let (input, hydration): (SessionInput, _) = match options.request {
        Some(path) => {
            let raw = read_bounded_file(&path, 1024 * 1024).map_err(protocol_error)?;
            (
                serde_json::from_value(parse_json(&raw)?)
                    .map_err(|e| Error::new("INVALID_SESSION_INPUT", e.to_string()))?,
                None,
            )
        }
        None => {
            let (input, hydration) = crate::arguments::prepare_session(
                crate::arguments::SessionPreparation {
                    after: options.after.ok_or_else(|| {
                        Error::new("INVALID_SELECTOR", "--after or --request is required")
                    })?,
                    data_dir: data_dir.clone(),
                    historical: options.historical,
                    controls: options.controls,
                    runtime: options.runtime,
                },
                cancellation,
            )?;
            (input, Some(hydration))
        }
    };
    // A restart after `--after` reopens the session from the journal's own retained initial
    // receipt (its pins are the journal's); this start's fresh preparation only supplies
    // hydration. A pinned request is its own input, which the journal must name.
    let input = match recorded {
        Some(recorded) if selector.is_some() => serde_json::from_value::<SessionInput>(recorded)
            .map_err(|e| Error::new("JOURNAL_CORRUPT", e.to_string()))?,
        _ => input,
    };
    let executable =
        std::env::current_exe().map_err(|e| Error::new("WORKER_OWNER", e.to_string()))?;
    let sha256 = file_sha256(&executable).map_err(protocol_error)?;
    let scratch = data_dir.join("scratch");
    std::fs::create_dir_all(&scratch).map_err(|e| Error::new("SESSION_SCRATCH", e.to_string()))?;
    let session = VerifiedSession::open(
        input,
        WorkerTransport::new(&scratch).with_owner(ProcessOwner { executable, sha256 }),
        WorkerLimits::default(),
        budget.clone(),
    )?;
    let mut session = match hydration {
        Some(hydration) => session.with_hydration(hydration)?,
        None => session,
    };
    if options.events {
        let event_budget = budget.clone();
        session.observe_transactions(move |event| {
            if let Err(error) = write_ready(&mut std::io::stdout().lock(), event, false) {
                eprintln!("SESSION_EVENT_OUTPUT: {error}");
                event_budget.cancel();
            }
        });
    }
    let Some((listener, socket)) = endpoints else {
        // The embedding tool reads one machine-readable ready line (after any journal
        // replay), then the replies.
        let ready =
            json!({"schema":"svm-historical-backend/v1","kind":"ready","pins":session.pins()});
        let journal = options.journal.map(|path| JournalFile {
            path,
            binding: options.journal_binding.unwrap_or_default(),
            after: selector,
        });
        let output = serve_backend(
            std::io::stdin(),
            std::io::stdout(),
            session,
            budget,
            ready,
            journal,
        )?;
        return Ok(
            json!({"schema":"svm-historical-backend/v1","outcome":"COMPLETED","kind":"closed",
            "pid":output.pid,"cleanup":format!("{:?}",output.cleanup_scope)}),
        );
    };
    let address = listener
        .local_addr()
        .map_err(|e| Error::new("SESSION_BIND", e.to_string()))?;
    let socket_address = socket
        .local_addr()
        .map_err(|e| Error::new("SESSION_BIND", e.to_string()))?;
    let ready = json!({"schema":"svm-historical-rpc/v1","kind":"ready","url":format!("http://{address}"),
        "wsUrl":format!("ws://{socket_address}"),"pins":session.pins()});
    {
        let mut output = std::io::stdout().lock();
        write_ready(&mut output, &ready, crate::terminal::enabled())
            .map_err(|e| Error::new("SESSION_OUTPUT", e.to_string()))?;
    }
    let output = match serve_session(listener, socket, session, budget.clone()) {
        Ok(output) => output,
        Err(error)
            if error.code == "WORKER_CANCELLED" && budget.cancellation_token().is_cancelled() =>
        {
            return Ok(
                json!({"schema":"svm-historical-rpc/v1", "kind":"interrupted",
                "outcome":"CANCELLED", "error":error}),
            );
        }
        Err(error) => return Err(error),
    };
    Ok(
        json!({"schema":"svm-historical-rpc/v1","outcome":"COMPLETED","kind":"closed",
        "pid":output.pid,"cleanup":format!("{:?}",output.cleanup_scope)}),
    )
}

/// HTTP on `port` and WebSocket on `port + 1`, where Solana clients look for it.
/// Port zero picks a free pair.
fn listeners(port: u16) -> Result<(TcpListener, TcpListener), Error> {
    let bind = |port: u16| TcpListener::bind(("127.0.0.1", port));
    let pair = |http: TcpListener| -> std::io::Result<(TcpListener, TcpListener)> {
        let next = http.local_addr()?.port().checked_add(1).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "no port above 65535")
        })?;
        Ok((http, bind(next)?))
    };
    if port != 0 {
        return bind(port).and_then(pair).map_err(|e| {
            Error::new(
                "SESSION_BIND",
                format!("ports {port} and {}: {e}", u32::from(port) + 1),
            )
        });
    }
    let mut last = None;
    for _ in 0..32 {
        match bind(0).and_then(pair) {
            Ok(listeners) => return Ok(listeners),
            Err(error) => last = Some(error),
        }
    }
    Err(Error::new(
        "SESSION_BIND",
        format!(
            "no free HTTP/WebSocket port pair: {}",
            last.map(|e| e.to_string()).unwrap_or_default()
        ),
    ))
}

fn bind_parent(expected: Option<u32>) -> Result<(), Error> {
    let Some(expected) = expected else {
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    unsafe {
        if expected <= 1 || libc::getppid() as u32 != expected {
            return Err(Error::new(
                "SESSION_PARENT",
                "SDK parent PID differs from the actual process parent",
            ));
        }
        if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
            return Err(Error::new(
                "SESSION_PARENT",
                format!("bind parent lifetime: {}", std::io::Error::last_os_error()),
            ));
        }
        if libc::getppid() as u32 != expected {
            return Err(Error::new(
                "SESSION_PARENT",
                "SDK parent exited during session startup",
            ));
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(Error::new(
            "UNSUPPORTED_PLATFORM",
            "SDK parent lifetime binding is currently qualified on Linux only",
        ))
    }
}

pub(super) fn write_ready(out: &mut impl Write, ready: &Value, human: bool) -> std::io::Result<()> {
    if !human {
        serde_json::to_writer(&mut *out, ready)?;
        out.write_all(b"\n")?;
    } else {
        use std::io::IsTerminal;
        let display = crate::terminal::display;
        let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        let pins = &ready["pins"];
        writeln!(out, "\nHistorical fork ready\n")?;
        for (label, value) in [
            ("RPC", &ready["url"]),
            ("WebSocket", &ready["wsUrl"]),
            ("Slot", &pins["slot"]),
            ("Transaction index", &pins["transactionIndex"]),
            ("Runtime", &pins["runtime"]["name"]),
        ] {
            let value = display(value);
            if color {
                writeln!(out, "  {label:18} \x1b[36m{value}\x1b[0m")?;
            } else {
                writeln!(out, "  {label:18} {value}")?;
            }
        }
        if let Some(warning) = pins.pointer("/runtimeSelection/warning") {
            writeln!(out, "\n  Warning: {}", display(warning))?;
        }
        writeln!(
            out,
            "\n  Clock/slot frozen; use svmReplay_advance to advance."
        )?;
        writeln!(out, "\nServing requests.\nPress Ctrl-C to stop.\n")?;
    }
    out.flush()
}
