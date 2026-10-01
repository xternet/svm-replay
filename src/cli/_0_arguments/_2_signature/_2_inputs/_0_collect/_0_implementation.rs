use super::*;
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct Collection {
    calls: bool,
    instructions: bool,
    registers: bool,
    memory: bool,
    program_ids: Vec<String>,
    instruction_indices: Vec<u64>,
    execution_mode: Option<String>,
    max_bytes: Option<u64>,
    max_events: Option<u64>,
    timeout_ms: Option<u64>,
    register_rows: Option<u64>,
    memory_rows: Option<u64>,
    max_invocations: Option<u64>,
}

pub(in super::super::super) fn collect(input: &str) -> Result<Option<TraceOptions>, Error> {
    let value = if input == "all" {
        serde_json::json!({"calls":true,"instructions":true,"registers":true,"memory":true})
    } else {
        json_argument(input)?
    };
    let c: Collection =
        serde_json::from_value(value).map_err(|e| Error::new("COLLECT_OPTIONS", e.to_string()))?;
    let sbpf = c.instructions || c.registers || c.memory;
    let enabled = c.calls || sbpf;
    let require_complete = [
        c.max_bytes,
        c.max_events,
        c.timeout_ms,
        c.register_rows,
        c.memory_rows,
        c.max_invocations,
    ]
    .iter()
    .all(Option::is_none);
    if (!sbpf && c.register_rows.is_some()) || (!c.memory && c.memory_rows.is_some()) {
        return Err(Error::new(
            "COLLECT_OPTIONS",
            "row limits require the corresponding capture category",
        ));
    }
    let mode =
        c.execution_mode
            .as_deref()
            .unwrap_or(if cfg!(all(target_arch = "x86_64", not(windows))) {
                "jit"
            } else {
                "interpreter"
            });
    if !["jit", "interpreter"].contains(&mode) {
        return Err(Error::new(
            "COLLECT_OPTIONS",
            "executionMode must be jit or interpreter",
        ));
    }
    // Omitted limits require a complete export. Adapter safety ceilings are not
    // permission to return partial data successfully.
    let value = serde_json::json!({
        "require_complete":require_complete,
        "capture": {
            "schema":"svm-capture-request/v2", "executionMode":mode,
            "level":if sbpf {"sbpf"} else if c.calls {"calls"} else {"off"},
            "sbpfObservations":if c.memory {"pc-registers-memory"} else if sbpf {"pc-registers"} else {"none"},
            "filter":{"programIds":c.program_ids,"instructionIndices":c.instruction_indices},
            "limits":{"maxBytes":c.max_bytes.unwrap_or(268435456),
                "maxEvents":c.max_events.unwrap_or(1000000),"timeoutMs":c.timeout_ms.unwrap_or(180000)}
        },
        "bounds":{"registerRows":if sbpf {c.register_rows.unwrap_or(2000000)} else {0},
            "memoryRows":if c.memory {Some(c.memory_rows.unwrap_or(2000000))} else {None},
            "maxInvocations":c.max_invocations.unwrap_or(1024)}
    });
    let options: TraceOptions =
        serde_json::from_value(value).map_err(|e| Error::new("COLLECT_OPTIONS", e.to_string()))?;
    options.bounds.validate(&options.capture)?;
    Ok(enabled.then_some(options))
}
