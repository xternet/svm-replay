use super::*;
use std::{
    fs::File,
    io::{Read as _, Write as _},
};

const SCHEMA: &str = "svm-historical-journal/v1";

/// What a journal needs of the session; the verified session is the real one.
pub trait Journaled {
    /// The session's boundary pins and the retained inputs it was opened from.
    fn journal_pins(&self) -> Value;
    fn journal_input(&self) -> Value;
    fn record_side_commands(&mut self);
    fn take_side_commands(&mut self) -> Vec<Value>;
    fn replay_side_commands(&mut self, sides: Vec<Value>);
    fn finish_replay(&mut self) -> Result<(), Error>;
    fn state_digest(&mut self, budget: &ExecutionBudget) -> Result<String, Error>;
}

impl Journaled for VerifiedSession {
    fn journal_pins(&self) -> Value {
        self.pins().clone()
    }
    fn journal_input(&self) -> Value {
        json!(self.input())
    }
    fn record_side_commands(&mut self) {
        VerifiedSession::record_side_commands(self)
    }
    fn take_side_commands(&mut self) -> Vec<Value> {
        VerifiedSession::take_side_commands(self)
    }
    fn replay_side_commands(&mut self, sides: Vec<Value>) {
        VerifiedSession::replay_side_commands(self, sides)
    }
    fn finish_replay(&mut self) -> Result<(), Error> {
        VerifiedSession::finish_replay(self)
    }
    fn state_digest(&mut self, budget: &ExecutionBudget) -> Result<String, Error> {
        VerifiedSession::state_digest(self, budget)
    }
}

fn integrity(code: &str, message: impl Into<String>) -> Error {
    Error::new(code, message)
}

/// Whether an operation changes a branch or the set of branches. The journal records those,
/// and also any read whose acquisitions (Hydrates) changed the fork's state.
pub(in super::super::super) fn changes_state(op: &Op) -> bool {
    match op {
        Op::Command(command) => matches!(
            command,
            SessionCommand::SetBalance { .. }
                | SessionCommand::OverrideAccounts { .. }
                | SessionCommand::RpcSend { .. }
                | SessionCommand::Send { .. }
                | SessionCommand::Advance { .. }
                | SessionCommand::Reset
        ),
        Op::Fork | Op::Close | Op::Simulate(_) | Op::Commit { .. } | Op::Restore { .. } => true,
        Op::Revision | Op::Peek { .. } | Op::KnownAccounts | Op::Archive(_) | Op::Shutdown => false,
    }
}

/// An operation's outcome as the journal compares it: its result's digest, or its error code.
fn outcome(result: &Result<Value, Error>) -> Value {
    match result {
        Ok(value) => json!({"ok":svm_replay_protocol::Digest::of(value.to_string().as_bytes())}),
        Err(error) => json!({"error":error.code}),
    }
}

/// Where a backend journals: the file, the embedder store it belongs to and the `--after`
/// selector the session was started from (none for a pinned request).
pub struct JournalFile {
    pub path: PathBuf,
    pub binding: String,
    pub after: Option<String>,
}

/// The backend's write-ahead record of committed operations: each one is appended and
/// synced before its reply is sent, and a restart replays them all through the backend.
pub struct Journal {
    file: File,
    records: u64,
    state: Option<String>,
    /// The newest embedder store revision among the records.
    revision: Option<u64>,
}

impl Journal {
    /// Opens the journal for this session. A new journal gets its header. An existing one
    /// must name the same selector, boundary pins and binding; its records are replayed in
    /// order, each reusing exactly its recorded Hydrates and reproducing its recorded outcome
    /// and state digest. Anything else refuses the start, nothing repaired.
    pub fn open(
        journal: &JournalFile,
        branches: &mut Branches,
        session: &mut (impl Commands + Archive + Journaled),
        budget: &ExecutionBudget,
    ) -> Result<Self, Error> {
        let path = journal.path.as_path();
        let (mut file, created) = super::super::_1_writer::open(path)?;
        session.record_side_commands();
        let header = json!({"schema":SCHEMA,"after":journal.after,"input":session.journal_input(),
            "pins":session.journal_pins(),"binding":journal.binding});
        let io = |e: std::io::Error| integrity("JOURNAL_IO", e.to_string());
        if created {
            writeln!(file, "{header}").map_err(io)?;
            file.sync_all().map_err(io)?;
            return Ok(Self {
                file,
                records: 0,
                state: None,
                revision: None,
            });
        }
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(io)?;
        if !text.ends_with('\n') {
            return Err(integrity(
                "JOURNAL_INCOMPLETE",
                "the journal ends inside a record that was never acknowledged",
            ));
        }
        let mut lines = text.lines();
        let found: Value = lines
            .next()
            .and_then(|line| serde_json::from_str(line).ok())
            .ok_or_else(|| integrity("JOURNAL_CORRUPT", "journal header unreadable"))?;
        // A restart reopens the session from the journal's own initial receipt, so its full
        // pins (receipt and prepared-request digests included) are the journal's exactly.
        if found["schema"] != SCHEMA
            || found["after"] != header["after"]
            || found["input"] != header["input"]
            || found["pins"] != header["pins"]
        {
            return Err(integrity(
                "JOURNAL_BOUNDARY_MISMATCH",
                "the journal belongs to another boundary, receipt or session",
            ));
        }
        if found["binding"] != header["binding"] {
            return Err(integrity(
                "JOURNAL_BINDING_MISMATCH",
                "the journal belongs to another embedder store",
            ));
        }
        let mut journal = Self {
            file,
            records: 0,
            state: None,
            revision: None,
        };
        for (seq, line) in lines.enumerate() {
            let corrupt = || integrity("JOURNAL_CORRUPT", format!("record {seq} is unreadable"));
            let record: Value = serde_json::from_str(line).map_err(|_| corrupt())?;
            let sides: Vec<Value> =
                serde_json::from_value(record["sides"].clone()).map_err(|_| corrupt())?;
            let op: Op = serde_json::from_value(record["op"].clone()).map_err(|_| corrupt())?;
            let branch = record["branch"].as_u64().ok_or_else(corrupt)?;
            let revision: Option<u64> =
                serde_json::from_value(record["revision"].clone()).map_err(|_| corrupt())?;
            if record["seq"] != json!(seq) {
                return Err(corrupt());
            }
            session.replay_side_commands(sides);
            let result =
                super::super::super::_3_serve::handle(branches, session, branch, op, budget);
            session.finish_replay().map_err(|error| {
                integrity(&error.code, format!("record {seq}: {}", error.message))
            })?;
            let state = session.state_digest(budget)?;
            if outcome(&result) != record["outcome"] || json!(state) != record["state"] {
                return Err(integrity(
                    "JOURNAL_DIVERGED",
                    format!("record {seq} no longer reproduces its recorded outcome and state"),
                ));
            }
            journal.records += 1;
            journal.state = Some(state);
            journal.revision = journal.revision.max(revision);
        }
        session.take_side_commands();
        Ok(journal)
    }

    /// Appends one operation (`entry`: its branch, raw op and embedder store revision) and
    /// syncs it before its reply is sent: an acknowledged operation is always in the journal.
    pub fn record(
        &mut self,
        entry: &Value,
        result: &Result<Value, Error>,
        sides: Vec<Value>,
        session: &mut impl Journaled,
        budget: &ExecutionBudget,
    ) -> Result<(), Error> {
        let state = session.state_digest(budget)?;
        let revision = entry["revision"].as_u64();
        let record = json!({"seq":self.records,"branch":entry["branch"],"op":entry["op"],
            "revision":revision,"sides":sides,"outcome":outcome(result),"state":state});
        let io = |e: std::io::Error| integrity("JOURNAL_IO", e.to_string());
        writeln!(self.file, "{record}").map_err(io)?;
        self.file.sync_data().map_err(io)?;
        self.records += 1;
        self.state = Some(state);
        self.revision = self.revision.max(revision);
        Ok(())
    }

    /// What a restart restored, for the embedder to compare with its own store: how many
    /// operations, the native state, and the newest store revision they belong to.
    pub fn summary(&self) -> Value {
        json!({"records":self.records,"state":self.state,"revision":self.revision})
    }

    /// How many operations the journal holds.
    pub fn records(&self) -> u64 {
        self.records
    }
}
