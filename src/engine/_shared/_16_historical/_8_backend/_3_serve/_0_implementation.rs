use super::*;

/// Above the worker's 16 MiB request frame, which bounds what can be forwarded: one
/// largest (10 MiB) account image in base64 plus its JSON envelope still fits.
const MAX_LINE: usize = 20 * 1024 * 1024;

enum Input {
    Line(String),
    Failed(String),
}

/// Serves one verified session to its embedding tool over `input`/`output` until the tool
/// sends `shutdown` or closes its end, then closes the worker it owns. With a journal, its
/// records are replayed first and every later committed operation is journaled before its
/// reply; the ready line follows the replay, and each reply carries the journal's count.
pub fn serve_backend(
    input: impl Read + Send + 'static,
    output: impl Write,
    mut session: VerifiedSession,
    budget: ExecutionBudget,
    ready: Value,
    journal: Option<JournalFile>,
) -> Result<WorkerOutput, Error> {
    let health = |session: &mut VerifiedSession| session.check();
    serve_lines(
        input,
        output,
        &mut session,
        &budget,
        health,
        Some(ready),
        journal,
    )?;
    session.close()
}

fn write_line(output: &mut impl Write, value: &Value) -> Result<(), Error> {
    serde_json::to_writer(&mut *output, value)
        .map_err(|e| e.to_string())
        .and_then(|()| output.write_all(b"\n").map_err(|e| e.to_string()))
        .and_then(|()| output.flush().map_err(|e| e.to_string()))
        .map_err(|e| Error::new("BACKEND_OUTPUT", e))
}

pub fn serve_lines<S: Commands + Archive + Journaled>(
    input: impl Read + Send + 'static,
    mut output: impl Write,
    session: &mut S,
    budget: &ExecutionBudget,
    mut health: impl FnMut(&mut S) -> Result<(), Error>,
    ready: Option<Value>,
    journal: Option<JournalFile>,
) -> Result<(), Error> {
    let mut branches = Branches::new(session.base_keys());
    let mut journal = match journal {
        Some(journal) => Some(Journal::open(&journal, &mut branches, session, budget)?),
        None => None,
    };
    if let Some(mut ready) = ready {
        if let Some(journal) = &journal {
            ready["journal"] = journal.summary();
        }
        write_line(&mut output, &ready)?;
    }
    let (lines, received) = sync_channel(1);
    std::thread::spawn(move || read_lines(input, lines));
    loop {
        let line = match received.recv_timeout(Duration::from_millis(50)) {
            Ok(Input::Line(line)) => line,
            Ok(Input::Failed(error)) => return Err(Error::new("BACKEND_INPUT", error)),
            Err(RecvTimeoutError::Timeout) => {
                budget.check().map_err(protocol_error)?;
                health(session)?;
                continue;
            }
            // The embedding tool exited or closed its end: nothing can be answered.
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        budget.check().map_err(protocol_error)?;
        let (mut reply, stop) = match _0_request::decode(&line) {
            Err((id, error)) => (json!({"id":id,"error":error}), false),
            Ok(Request {
                id,
                branch,
                op,
                revision,
            }) => {
                let stop = matches!(op, Op::Shutdown);
                let changes = changes_state(&op);
                let result = handle(&mut branches, session, branch, op, budget);
                if let Some(journal) = &mut journal {
                    let sides = session.take_side_commands();
                    if changes || !sides.is_empty() {
                        let request: Value = serde_json::from_str(&line)
                            .map_err(|e| Error::new("BACKEND_INPUT", e.to_string()))?;
                        // The operation is journaled with its embedder store revision, if any.
                        let entry = json!({"branch":branch,"op":request["op"],"revision":revision});
                        journal.record(&entry, &result, sides, session, budget)?;
                    }
                }
                match result {
                    Ok(result) => (json!({"id":id,"result":result}), stop),
                    Err(error) => (json!({"id":id,"error":error}), stop),
                }
            }
        };
        // The embedder records how much of the journal its own store has seen.
        if let Some(journal) = &journal {
            reply["journaled"] = json!(journal.records());
        }
        write_line(&mut output, &reply)?;
        if stop {
            return Ok(());
        }
    }
}

pub(in super::super) fn handle(
    branches: &mut Branches,
    session: &mut (impl Commands + Archive),
    branch: u64,
    op: Op,
    budget: &ExecutionBudget,
) -> Result<Value, Error> {
    match op {
        Op::Command(command) => branches.run(session, branch, command, budget),
        Op::Fork => Ok(json!({"branch": branches.fork(session, branch, budget)?})),
        Op::Close => {
            branches.close(session, branch, budget)?;
            Ok(json!({"closed": branch}))
        }
        Op::Simulate(simulation) => {
            super::super::_2_simulate::simulate(branches, session, branch, simulation, budget)
        }
        Op::Commit { into } => {
            branches.commit(session, branch, into, budget)?;
            Ok(json!({"branch": branch, "replaced": into}))
        }
        Op::Revision => Ok(json!({"revision": branches.revision(branch)?})),
        Op::KnownAccounts => {
            super::super::_5_inventory::known_accounts(branches, session, branch, budget)
        }
        Op::Restore { pubkeys } => {
            super::super::_4_restore::restore(branches, session, branch, pubkeys, budget)
        }
        Op::Peek { pubkeys } => {
            if pubkeys.len() > 100 {
                return Err(Error::new("INVALID_REQUEST", "at most 100 account peeks"));
            }
            branches.enter(session, branch, budget)?;
            session.peek(pubkeys, budget)
        }
        Op::Archive(query) => super::super::_6_archive::read(session, query),
        Op::Shutdown => Ok(json!({"shutdown": true})),
    }
}

fn read_lines(input: impl Read, lines: SyncSender<Input>) {
    let mut reader = BufReader::new(input);
    loop {
        let mut line = Vec::new();
        let read = (&mut reader)
            .take(MAX_LINE as u64 + 1)
            .read_until(b'\n', &mut line);
        let message = match read {
            Ok(0) => return,
            Ok(_) if line.len() > MAX_LINE => Input::Failed("request line exceeds 20 MiB".into()),
            Ok(_) => match String::from_utf8(line) {
                Ok(text) if text.trim().is_empty() => continue,
                Ok(text) => Input::Line(text),
                Err(_) => Input::Failed("request line is not UTF-8".into()),
            },
            Err(error) => Input::Failed(format!("read request: {error}")),
        };
        let failed = matches!(message, Input::Failed(_));
        if lines.send(message).is_err() || failed {
            return;
        }
    }
}
