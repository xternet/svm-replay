use super::*;
use serde::{Deserialize, Serialize};

/// Local immutable inputs; credentials and acquisition configuration are absent.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionInput {
    pub schema: String,
    pub receipt_path: PathBuf,
    pub receipt_sha256: String,
    pub worker_path: PathBuf,
    pub after: bool,
}

pub struct VerifiedSession {
    pub(in super::super) input: SessionInput,
    pub(in super::super) hydration: Option<SessionHydration>,
    pub(in super::super) native: NativeSession,
    pub(in super::super) pins: Value,
    pub(in super::super) ready: Value,
    pub(in super::super) observer: Option<Box<dyn FnMut(&Value) + Send>>,
    /// What the boundary fork holds: the fixture's accounts and its Clock, as the worker
    /// loads them.
    pub(in super::super) base_keys: Vec<String>,
    /// Keys hydration loaded into the active fork since an embedder last took them.
    pub(in super::super) hydrated_keys: Vec<String>,
    /// The boundary block as archived (the provider's getBlock result), source-pinned.
    pub(in super::super) boundary: std::sync::Arc<Value>,
    /// The verified Hydrates this session sent while a journal records.
    pub(in super::super) side_log: Option<Vec<Value>>,
    /// A journal replay's recorded Hydrates, used instead of acquiring again.
    pub(in super::super) side_replay: std::collections::VecDeque<Value>,
    /// Whether a journal replay runs: every acquisition then comes from the journal.
    pub(in super::super) replaying: bool,
    /// The pinned parent headers the worker accepted as the boundary queue's proof, newest
    /// first.
    pub(in super::super) queue_headers: Vec<Value>,
}

/// The account keys a fixture makes known to a fork: its accounts and its Clock.
pub(in super::super) fn fixture_keys(fixture: &Value) -> Result<Vec<String>, Error> {
    let key = |account: &Value| {
        account["pubkey"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Error::new("SESSION_PROTOCOL", "fixture account key missing"))
    };
    let accounts = fixture["accounts"]
        .as_array()
        .ok_or_else(|| Error::new("SESSION_PROTOCOL", "fixture accounts missing"))?;
    let mut keys = accounts.iter().map(key).collect::<Result<Vec<_>, _>>()?;
    keys.push(key(&fixture["clock"])?);
    Ok(keys)
}

pub(in super::super::super) struct Prepared {
    pub fixture: Value,
    pub original: Value,
    pub worker: WorkerSpec,
    pub pins: Value,
    pub rpc_context: Value,
    pub initialized_state_sha256: Option<String>,
    pub boundary: Value,
}
