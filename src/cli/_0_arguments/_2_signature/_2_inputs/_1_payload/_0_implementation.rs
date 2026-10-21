use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use svm_replay_protocol::{StateOverride, WhatIf, WhatIfMode, MAX_BUNDLE};

/// Loader that runs an ELF placed in one account at once (no deployment delay).
const PROGRAM_OWNER: &str = "BPFLoader2111111111111111111111111111111111";
const MAX_PROGRAM: usize = 10 * 1024 * 1024;

pub(in super::super::super) fn json_argument(input: &str) -> Result<Value, Error> {
    let bytes = if let Some(path) = input.strip_prefix('@') {
        if path.is_empty() {
            return Err(Error::new("INVALID_REQUEST", "@ requires a file path"));
        }
        read_bounded_file(Path::new(path), 16 * 1024 * 1024)
            .map_err(|e| Error::new("INPUT_IO", e.to_string()))?
    } else {
        input.as_bytes().to_vec()
    };
    parse_json(&bytes)
}

/// `--replace` / `--append`: one `{"transactionBase64": …}` object or an array of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super::super) struct Bundle {
    pub transactions: Vec<String>,
    /// Given as an array: a bundle even with one transaction.
    pub array: bool,
}

pub(in super::super::super) fn bundle(input: &str) -> Result<Bundle, Error> {
    let invalid = |message: &str| Error::new("INVALID_REQUEST", message);
    let value = json_argument(input)?;
    let (rows, array) = match value {
        Value::Array(rows) => (rows, true),
        other => (vec![other], false),
    };
    if rows.is_empty() || rows.len() > MAX_BUNDLE {
        return Err(invalid(&format!(
            "a bundle has 1 to {MAX_BUNDLE} transactions"
        )));
    }
    let mut transactions = Vec::new();
    for row in rows {
        let wire = row
            .as_object()
            .filter(|v| v.len() == 1)
            .and_then(|v| v.get("transactionBase64"))
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("each transaction is {\"transactionBase64\": \"…\"}"))?;
        svm_replay_protocol::transaction::decode(wire)?;
        transactions.push(wire.to_owned());
    }
    Ok(Bundle {
        transactions,
        array,
    })
}

/// `--overrides`: 0.1.0-shaped entries stay on the 0.1.0 path; any other field makes them
/// eth_call-style state overrides.
#[derive(Clone, Debug, PartialEq)]
pub(in super::super::super) enum Overrides {
    Legacy(Value),
    State(Vec<StateOverride>),
}

pub(in super::super::super) fn overrides(input: &str) -> Result<Overrides, Error> {
    let value = json_argument(input)?;
    let rows = value
        .as_array()
        .ok_or_else(|| Error::new("INVALID_REQUEST", "overrides must be a JSON array"))?;
    let legacy = rows.iter().all(|row| {
        row.as_object().is_some_and(|row| {
            row.keys()
                .all(|key| ["pubkey", "lamports", "dataBase64"].contains(&key.as_str()))
        })
    });
    if legacy {
        let patches =
            svm_replay_engine::shared::dependencies::overrides::validate_requested_overrides(
                &value,
            )?;
        for patch in patches {
            svm_replay_engine::shared::history::address(&patch.pubkey, 32)?;
        }
        return Ok(Overrides::Legacy(value));
    }
    rows.iter()
        .map(state_override)
        .collect::<Result<_, _>>()
        .map(Overrides::State)
}

fn state_override(row: &Value) -> Result<StateOverride, Error> {
    let invalid = |message: String| Error::new("INVALID_STATE_OVERRIDE", message);
    let mut row = row
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("each override is an object".into()))?;
    if let Some(program) = row.remove("program") {
        let path = program
            .as_str()
            .and_then(|path| path.strip_prefix('@'))
            .filter(|path| !path.is_empty())
            .ok_or_else(|| invalid("program is \"@path/to/program.so\"".into()))?;
        if ["owner", "executable", "dataBase64"]
            .iter()
            .any(|key| row.contains_key(*key))
        {
            return Err(invalid(
                "program sets owner, executable and dataBase64 itself".into(),
            ));
        }
        let bytes = read_bounded_file(Path::new(path), MAX_PROGRAM)
            .map_err(|e| Error::new("INPUT_IO", format!("{path}: {e}")))?;
        if !bytes.starts_with(b"\x7fELF") {
            return Err(invalid(format!("{path} is not a program (.so ELF file)")));
        }
        row.insert("owner".into(), PROGRAM_OWNER.into());
        row.insert("executable".into(), true.into());
        row.insert("dataBase64".into(), STANDARD.encode(bytes).into());
    }
    let entry: StateOverride =
        serde_json::from_value(Value::Object(row)).map_err(|e| invalid(e.to_string()))?;
    entry.validate()?;
    Ok(entry)
}

/// Request fields for the requested changes: 0.1.0 replacement, 0.1.0 overrides, what-if bundle.
pub(in super::super::super) type Variants = (Option<String>, Option<Value>, Option<WhatIf>);

/// Picks the request fields: the 0.1.0 replacement / overrides when the input has their shape,
/// otherwise a what-if bundle (`--append` runs after the target, `--replace` instead of it).
pub(in super::super::super) fn variants(
    replace: Option<Bundle>,
    append: Option<Bundle>,
    overrides: Option<Overrides>,
) -> Result<Variants, Error> {
    let state = matches!(overrides, Some(Overrides::State(_)));
    if append.is_none() && !replace.as_ref().is_some_and(|b| b.array) && !state {
        let replacement = replace.and_then(|b| b.transactions.into_iter().next());
        let legacy = match overrides {
            Some(Overrides::Legacy(value)) => Some(value),
            _ => None,
        };
        return Ok((replacement, legacy, None));
    }
    let state_overrides = match overrides {
        None => Vec::new(),
        Some(Overrides::State(entries)) => entries,
        Some(Overrides::Legacy(value)) => serde_json::from_value(value)
            .map_err(|e| Error::new("INVALID_STATE_OVERRIDE", e.to_string()))?,
    };
    let (mode, transactions) = match (append, replace) {
        (Some(_), Some(_)) => {
            return Err(Error::new(
                "INVALID_REQUEST",
                "--append and --replace cannot be combined",
            ))
        }
        (Some(bundle), None) => (WhatIfMode::After, Some(bundle.transactions)),
        (None, Some(bundle)) => (WhatIfMode::Replace, Some(bundle.transactions)),
        (None, None) => (WhatIfMode::Replace, None),
    };
    let what_if = WhatIf {
        mode,
        transactions_base64: transactions,
        state_overrides,
    };
    what_if.validate()?;
    Ok((None, None, Some(what_if)))
}
