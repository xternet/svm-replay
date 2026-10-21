use super::*;

impl NativeSession {
    /// Open an already prepared fixture under its pinned worker and owner.
    /// The lifetime budget is shared, never renewed per command. This transport
    /// does not acquire history or prove the control: its caller must verify
    /// `ready().original` before exposing a historical session to a tool.
    pub fn open(
        transport: WorkerTransport,
        worker: WorkerSpec,
        fixture: Value,
        after: bool,
        limits: WorkerLimits,
        budget: ExecutionBudget,
    ) -> Result<Self, Error> {
        budget.check().map_err(protocol_error)?;
        let initialized = fixture["schema"] == "svm-historical-initialized-slot/v1";
        let slot = fixture["target"]["targetSlot"]
            .as_u64()
            .ok_or_else(|| error("session target slot missing"))?;
        if initialized && (after || fixture["target"]["kind"] != "initialized-slot") {
            return Err(error("initialized slots have no after-transaction phase"));
        }
        // Control IPC only. Historical worker input/output still uses the strict
        // cold scratch root owned by WorkerTransport, without any /tmp fallback.
        let ipc = tempfile::Builder::new()
            .prefix("svm-ipc-")
            .tempdir()
            .map_err(|e| error(format!("create private command directory: {e}")))?;
        let socket = ipc.path().join("ipc");
        let listener =
            UnixListener::bind(&socket).map_err(|e| error(format!("bind session: {e}")))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| error(format!("session listener: {e}")))?;
        let socket_arg = socket
            .to_str()
            .ok_or_else(|| error("session socket path must be UTF-8"))?
            .to_owned();
        let flags = vec![
            if initialized {
                "--session-initialized"
            } else if after {
                "--session-after"
            } else {
                "--session-before"
            }
            .to_owned(),
            socket_arg,
        ];
        let worker_budget = budget.clone();
        let supervisor = std::thread::Builder::new()
            .name("svm-session-owner".into())
            .spawn(move || transport.run(&worker, &fixture, &flags, &limits, &worker_budget))
            .map_err(|e| error(format!("start native session supervisor: {e}")))?;
        let mut session = Self {
            wire: None,
            supervisor: Some(supervisor),
            ipc: Some(ipc),
            budget,
            ready: Value::Null,
            next_id: 1,
        };
        let start = (|| {
            loop {
                session.budget.check().map_err(protocol_error)?;
                match listener.accept() {
                    Ok((stream, _)) => {
                        session.wire = Some(_1_frames::Wire::new(stream, 64 * 1024 * 1024 + 1)?);
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if session
                            .supervisor
                            .as_ref()
                            .is_some_and(|worker| worker.is_finished())
                        {
                            return Err(error("native session exited before command connection"));
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => return Err(error(format!("accept native session: {e}"))),
                }
            }
            let ready = session
                .wire
                .as_mut()
                .ok_or_else(|| error("session channel missing"))?
                .read(&session.budget, &session.budget)?;
            _1_ready::validate(&ready, initialized, slot)?;
            session.ready = ready;
            Ok(())
        })();
        match start {
            Ok(()) => Ok(session),
            Err(primary) => Err(session.abort(primary)),
        }
    }
}
