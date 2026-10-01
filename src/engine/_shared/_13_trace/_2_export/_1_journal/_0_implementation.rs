use super::*;

pub(in super::super) struct Frame {
    pub(in super::super) id: u64,
    pub(in super::super) parent: Option<u64>,
    pub(in super::super) start: u64,
    pub(in super::super) children: u64,
    pub(in super::super) omitted: bool,
}

pub(in super::super) fn validate_journal(
    row: &Value,
    events: &[Value],
    request: &CaptureRequest,
) -> Result<(), Error> {
    let complete = row["journal"]["disposition"] == "COMPLETE";
    string(field(row, "scope")?)?;
    string(field(row, "before_scope")?)?;
    if complete {
        check(
            events.len() >= 2
                && row["journal"]["reason"].is_null()
                && row["terminal_scope"].is_string()
                && events
                    .first()
                    .is_some_and(|event| event["kind"] == "transaction_begin")
                && events
                    .last()
                    .is_some_and(|event| event["kind"] == "transaction_commit"),
            "complete journal lacks transaction boundaries",
        )?;
    }
    let mut stack: Vec<Frame> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut began = false;
    let mut committed = false;
    for (index, event) in events.iter().enumerate() {
        check(
            event.is_object() && !committed,
            "malformed event or event after commit",
        )?;
        let current_parent = stack.last().map(|frame| frame.id);
        match string(field(event, "kind")?)? {
            "transaction_begin" => {
                check(index == 0 && !began, "transaction begin order differs")?;
                began = true;
                accounts(field(event, "accounts")?, Some(field(row, "before_keys")?))?;
            }
            kind => {
                check(began, "event before transaction boundary")?;
                match kind {
                    "transaction_commit" => {
                        check(stack.is_empty(), "transaction commit while invocation open")?;
                        outcome(field(event, "outcome")?)?;
                        accounts(
                            field(event, "accounts")?,
                            Some(field(row, "terminal_keys")?),
                        )?;
                        committed = true;
                    }
                    "enter" | "enter_omitted" => {
                        let id = unsigned(field(event, "call_id")?, false)?;
                        let parent = parent(field(event, "parent_id")?)?;
                        check(
                            seen.insert(id) && parent == current_parent,
                            "invocation identity/parent order differs",
                        )?;
                        let invocation = field(event, "invocation")?;
                        let program = string(field(invocation, "program_id")?)?;
                        key(program)?;
                        let instruction =
                            unsigned(field(invocation, "outer_instruction_index")?, false)?;
                        check(instruction <= u32::MAX as u64, "outer instruction overflow")?;
                        let selected = (request.filter.program_ids.is_empty()
                            || request
                                .filter
                                .program_ids
                                .iter()
                                .any(|value| value == program))
                            && (request.filter.instruction_indices.is_empty()
                                || request.filter.instruction_indices.contains(&instruction));
                        let omitted = kind == "enter_omitted";
                        check(
                            selected != omitted,
                            "call payload differs from requested filter",
                        )?;
                        if omitted {
                            string(field(event, "payload_omission")?)?;
                        } else {
                            accounts(field(invocation, "accounts")?, None)?;
                            raw_bytes(field(invocation, "instruction_data")?)?;
                        }
                        stack.push(Frame {
                            id,
                            parent,
                            start: unsigned(field(invocation, "remaining_cu")?, false)?,
                            children: 0,
                            omitted,
                        });
                    }
                    "exit" | "exit_omitted" => {
                        let frame = stack
                            .pop()
                            .ok_or_else(|| invalid("exit without invocation"))?;
                        check(
                            frame.id == unsigned(field(event, "call_id")?, false)?
                                && frame.parent == parent(field(event, "parent_id")?)?
                                && frame.omitted == (kind == "exit_omitted"),
                            "invocation exit nesting/payload differs",
                        )?;
                        let completion = field(event, "completion")?;
                        outcome(field(completion, "outcome")?)?;
                        let used = frame
                            .start
                            .checked_sub(unsigned(field(completion, "remaining_cu")?, false)?)
                            .ok_or_else(|| invalid("CU increased during call"))?;
                        let exclusive = used
                            .checked_sub(frame.children)
                            .ok_or_else(|| invalid("child CU exceeds parent"))?;
                        check(
                            unsigned(field(event, "inclusive_cu")?, false)? == used
                                && unsigned(field(event, "exclusive_cu")?, false)? == exclusive,
                            "per-call CU arithmetic differs",
                        )?;
                        if let Some(parent) = stack.last_mut() {
                            parent.children = parent
                                .children
                                .checked_add(used)
                                .ok_or_else(|| invalid("child CU overflow"))?;
                        }
                        if !frame.omitted {
                            accounts(field(completion, "accounts")?, None)?;
                            let returned = field(completion, "return_data")?;
                            if !returned.is_null() {
                                key(string(field(returned, "program_id")?)?)?;
                                raw_bytes(field(returned, "data")?)?;
                            }
                        }
                    }
                    "rejected" | "cpi_attempt_rejected" => {
                        check(
                            parent(field(event, "parent_id")?)? == current_parent,
                            "rejected attempt parent differs",
                        )?;
                        let rejection = if kind == "rejected" {
                            field(event, "rejection")?
                        } else {
                            event
                        };
                        string(field(rejection, "error")?)?;
                        unsigned(field(rejection, "remaining_cu")?, false)?;
                        let detail = if kind == "rejected" {
                            rejection
                        } else {
                            field(event, "attempt")?
                        };
                        check(
                            unsigned(field(detail, "outer_instruction_index")?, false)?
                                <= u32::MAX as u64,
                            "outer instruction overflow",
                        )?;
                        if !field(detail, "program_id")?.is_null() {
                            key(string(field(detail, "program_id")?)?)?;
                        }
                        if kind == "cpi_attempt_rejected" {
                            string(field(detail, "phase")?)?;
                            let payload = field(detail, "payload")?;
                            let disposition = string(field(payload, "capture")?)?;
                            check(
                                ["captured", "omitted_by_filter", "unavailable"]
                                    .contains(&disposition),
                                "CPI attempt disposition missing",
                            )?;
                            if disposition == "captured" {
                                raw_bytes(field(payload, "instruction_data")?)?;
                                for account in array(field(payload, "accounts")?)? {
                                    key(string(field(account, "pubkey")?)?)?;
                                    check(
                                        field(account, "is_signer")?.is_boolean()
                                            && field(account, "is_writable")?.is_boolean(),
                                        "CPI account privilege absent",
                                    )?;
                                }
                            }
                        }
                    }
                    _ => return Err(invalid(format!("unknown/failed journal event {kind}"))),
                }
            }
        }
    }
    check(
        !complete || committed && stack.is_empty(),
        "incomplete journal claimed COMPLETE",
    )
}
