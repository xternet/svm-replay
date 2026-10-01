use serde_json::Value;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};
use svm_replay_protocol::Error;

// One CLI invocation selects presentation once, before any execution.
static PRETTY: AtomicBool = AtomicBool::new(false);
static PRETTY_FILE: AtomicBool = AtomicBool::new(false);

pub(crate) fn configure(pretty: bool, machine: bool) {
    PRETTY.store(
        pretty || (!machine && std::io::stdout().is_terminal()),
        Ordering::Relaxed,
    );
    PRETTY_FILE.store(pretty, Ordering::Relaxed);
}

pub(crate) fn encode(value: &Value) -> Result<Vec<u8>, Error> {
    serialize(value, PRETTY.load(Ordering::Relaxed))
}

pub(crate) fn encode_file(value: &Value) -> Result<Vec<u8>, Error> {
    serialize(value, PRETTY_FILE.load(Ordering::Relaxed))
}

fn serialize(value: &Value, pretty: bool) -> Result<Vec<u8>, Error> {
    let bytes = if pretty {
        serde_json::to_vec_pretty(value)
    } else {
        serde_json::to_vec(value)
    };
    bytes.map_err(|error| Error::new("CLI_OUTPUT", error.to_string()))
}
