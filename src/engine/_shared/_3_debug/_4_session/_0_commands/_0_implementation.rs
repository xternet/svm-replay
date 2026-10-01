use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PauseToken {
    pub execution_index: u16,
    pub invocation_index: u16,
    pub generation: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DebugAction {
    Registers,
    Memory {
        address: u64,
        length: usize,
    },
    Metadata,
    SourceLocation,
    Frames,
    SourceVariable {
        name: String,
    },
    SourceBreakpoint {
        file: String,
        line: u32,
        enabled: bool,
    },
    SourceNext,
    SourceFinish,
    AccountDirectory,
    InspectAccount {
        pubkey: String,
        max_data_bytes: usize,
    },
    Breakpoint {
        pc: u64,
        enabled: bool,
    },
    Step,
    Continue,
    Pause,
    StepOver,
    StepOut,
    ContinueAll,
    Detach,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DebugCommand {
    pub request_id: u64,
    pub phase: String,
    pub target: Option<PauseToken>,
    pub action: DebugAction,
}

pub struct DebugController {
    pub(in super::super) commands: SyncSender<DebugCommand>,
    pub(in super::super) events: Receiver<Value>,
    pub(in super::super) failure: Arc<Mutex<Option<Error>>>,
}

pub struct DebugDriver {
    pub(in super::super) commands: Receiver<DebugCommand>,
    pub(in super::super) events: SyncSender<Value>,
    pub(in super::super) request_ids: BTreeSet<u64>,
    pub(in super::super) event_count: usize,
    pub(in super::super) event_bytes: usize,
    pub(in super::super) failure: Arc<Mutex<Option<Error>>>,
    pub(in super::super) symbols: BTreeMap<String, ExactSymbols>,
}

pub fn channel() -> (DebugController, DebugDriver) {
    let (send_command, commands) = mpsc::sync_channel(64);
    let (events, receive_event) = mpsc::sync_channel(256);
    let failure = Arc::new(Mutex::new(None));
    (
        DebugController {
            commands: send_command,
            events: receive_event,
            failure: failure.clone(),
        },
        DebugDriver {
            commands,
            events,
            request_ids: BTreeSet::new(),
            event_count: 0,
            event_bytes: 0,
            failure,
            symbols: BTreeMap::new(),
        },
    )
}
