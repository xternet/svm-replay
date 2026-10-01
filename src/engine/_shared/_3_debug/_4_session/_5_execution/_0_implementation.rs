use super::*;

pub struct LiveExecution {
    pub worker: WorkerOutput,
    pub events: Vec<Value>,
    pub invocations: Vec<InvocationMetadata>,
}

/// Executes a single pinned worker while servicing only bounded live commands.
/// Listener notices trigger attachment; only validated RSP metadata establishes VM identity.
pub fn run_session(
    transport: &WorkerTransport,
    worker: &WorkerSpec,
    fixture: &Value,
    flags: &[String],
    limits: &WorkerLimits,
    budget: &ExecutionBudget,
    port: u16,
    max_invocations: usize,
    phase: &str,
    driver: &mut DebugDriver,
) -> Result<LiveExecution, Error> {
    require(
        port > 0 && (1..=1024).contains(&max_invocations) && !phase.is_empty() && phase.len() <= 64,
        "DEBUG_CONFIG",
        "invalid live session limits",
    )?;
    let (notice_sender, notices) = mpsc::sync_channel(64);
    let (result_sender, results) = mpsc::sync_channel(1);
    let mut router = Router {
        invocations: BTreeMap::new(),
        automatic: BTreeSet::new(),
        all: false,
        executions: BTreeSet::new(),
        max_invocations,
    };
    let mut events = Vec::new();
    let result = thread::scope(|scope| {
        let worker_thread = scope.spawn(|| {
            let execution = transport.run_with_diagnostics(
                worker,
                fixture,
                flags,
                &[],
                limits,
                budget,
                &notice_sender,
            );
            result_sender.send(execution).map_err(|e| {
                error(
                    "DEBUG_CHANNEL",
                    format!("worker completion channel disconnected: {e}"),
                )
            })
        });
        let mut partial = Vec::new();
        let mut worker_result = None;
        let interactive = (|| -> Result<(), Error> {
            driver.emit(
                json!({"kind":"phase-start","phase":phase,"executionMode":"interpreter-debug"}),
                &mut events,
            )?;
            loop {
                budget.check().map_err(protocol_error)?;
                for _ in 0..64 {
                    match notices.try_recv() {
                        Ok(bytes) => {
                            partial.extend(bytes);
                            require(
                                partial.len() <= limits.max_diagnostic_bytes,
                                "DEBUG_LIMIT",
                                "listener diagnostic buffer exhausted",
                            )?;
                            while let Some(end) = partial.iter().position(|b| *b == b'\n') {
                                let line = partial.drain(..=end).collect::<Vec<_>>();
                                if line.starts_with(b"Waiting for bounded debugger on ") {
                                    require(
                                        line == format!(
                                            "Waiting for bounded debugger on 127.0.0.1:{port}\n"
                                        )
                                        .as_bytes(),
                                        "DEBUG_ENDPOINT",
                                        "unexpected or non-loopback debugger listener notice",
                                    )?;
                                    let mut client = DebugClient::connect(port, 32768, budget)?;
                                    let metadata = InvocationMetadata::parse(&client.metadata()?)?;
                                    router.accept(client, metadata, phase, driver, &mut events)?;
                                }
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => break,
                    }
                }
                router.poll(phase, driver, &mut events)?;
                for _ in 0..64 {
                    match driver.commands.try_recv() {
                        Ok(command) => router.command(command, phase, driver, &mut events)?,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            return Err(error("CANCELLED", "debug controller disconnected"))
                        }
                    }
                }
                match results.try_recv() {
                    Ok(result) => {
                        worker_result = Some(result);
                        return Ok(());
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        return Err(error("DEBUG_CHANNEL", "worker result disappeared"))
                    }
                }
                thread::sleep(Duration::from_millis(2));
            }
        })();
        if interactive.is_err() {
            budget.cancel();
        }
        let mut cleanup_errors = Vec::new();
        for invocation in router.invocations.values_mut() {
            if let Err(e) = invocation.client.close() {
                cleanup_errors.push(e.to_string());
            }
        }
        match worker_thread.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => cleanup_errors.push(e.to_string()),
            Err(_) => cleanup_errors.push("worker transport thread panicked".into()),
        }
        let observed = match worker_result {
            Some(result) => result,
            None => results.try_recv().map_err(|e| {
                error(
                    "DEBUG_CHANNEL",
                    format!("worker completion unavailable after join: {e}"),
                )
            })?,
        };
        if !cleanup_errors.is_empty() {
            return Err(error("DEBUG_CLEANUP",cleanup_errors.join("; ")).with_details(json!({"cause":interactive.err(),"workerError":observed.err().map(protocol_error)})));
        }
        if let Err(primary) = interactive {
            return Err(primary.with_details(
                json!({"workerError":observed.err().map(protocol_error),"events":events}),
            ));
        }
        let worker = observed.map_err(protocol_error)?;
        require(
            router.invocations.values().all(|vm| {
                matches!(
                    vm.client.state(),
                    DebugState::VmExited | DebugState::VmTerminated
                )
            }),
            "DEBUG_ROUTING",
            "worker completed with an active or cancelled VM connection",
        )?;
        require(
            !partial.starts_with(b"Waiting for bounded debugger on "),
            "DEBUG_PROTOCOL",
            "unterminated listener notice",
        )?;
        Ok(worker)
    });
    result.map(|worker| LiveExecution {
        worker,
        events,
        invocations: router
            .invocations
            .into_values()
            .map(|vm| vm.metadata)
            .collect(),
    })
}
