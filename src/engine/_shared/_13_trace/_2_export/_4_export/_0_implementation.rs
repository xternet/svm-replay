use super::*;

/// Validate producer observations before bounded export, never certify historical inputs.
pub fn make_export(
    request: &CaptureRequest,
    identity: &Digest,
    output: &Value,
) -> Result<CaptureExport, Error> {
    request.validate()?;
    check(
        request.level != "off" && request.limits.max_bytes >= 2,
        "enabled capture requires at least two export bytes",
    )?;
    check(
        output["schema"] == "svm-historical-capture-worker-experimental/v1"
            && output["status"] == "COMPLETE",
        "capture worker not finalized",
    )?;
    validate_execution_mode(output, &request.execution_mode)?;
    let journals = array(field(output, "journals")?)?;
    check(!journals.is_empty(), "finalized journals absent")?;
    let captures = array(field(output, "captures")?)?;
    let mut payload = Payload {
        bytes: vec![b'['],
        events: 0,
        exhausted: false,
    };
    let mut reasons = Vec::new();
    let mut partial = false;
    for (index, row) in journals.iter().enumerate() {
        let journal = field(row, "journal")?;
        let disposition = string(field(journal, "disposition")?)?;
        check(
            ["COMPLETE", "PARTIAL", "TRUNCATED"].contains(&disposition),
            "runtime journal failed/malformed",
        )?;
        let raw = string(field(journal, "payload_json")?)?;
        let events = parse_json(raw.as_bytes())?;
        let events = array(&events)?;
        check(
            events.len() as u64 == unsigned(field(journal, "event_count")?, false)?,
            "journal event count differs",
        )?;
        validate_journal(row, events, request)?;
        if disposition != "COMPLETE" {
            let reason = field(journal, "reason")?;
            check(
                reason.is_null() || reason.as_str().is_some_and(|text| !text.trim().is_empty()),
                "invalid partial journal reason",
            )?;
            reasons.push(format!("journal {index}: {disposition}: {reason}"));
            partial |= disposition == "PARTIAL";
        }
        let mut scope = json!({"kind":"journal_scope","journalIndex":index});
        for field_name in [
            "signatures",
            "scope",
            "before_scope",
            "before_keys",
            "terminal_scope",
            "terminal_keys",
        ] {
            scope[field_name] = field(row, field_name)?.clone();
        }
        payload.append(scope, request)?;
        for event in events {
            payload.append(
                json!({"kind":"journal_event","journalIndex":index,"event":event}),
                request,
            )?;
        }
    }
    if request.level == "sbpf" {
        let mut ordinals = BTreeSet::new();
        for capture in captures {
            check(
                capture["execution_mode"] == request.execution_mode
                    && ["COMPLETE", "TRUNCATED"].contains(&string(field(capture, "disposition")?)?),
                "SBPF mode/disposition differs",
            )?;
            validate_sbpf(capture)?;
            let ordinal = unsigned(field(capture, "ordinal")?, true)?;
            check(ordinals.insert(ordinal), "duplicate SBPF execution ordinal")?;
            if capture["disposition"] == "TRUNCATED" {
                reasons.push(format!("execution {ordinal}: producer truncation"));
            }
            payload.append(json!({"kind":"execution_scope","executionOrdinal":ordinal,"signatures":field(capture,"signatures")?,
                "executionMode":capture["execution_mode"],"invocations":capture["invocations"],"scope":"SBPF VM only; no native register path"}),request)?;
            let registers = array(field(capture, "trace")?)?;
            let memory_rows = if request.sbpf_observations == "pc-registers-memory" {
                let memory = field(capture, "memory")?;
                check(
                    ["COMPLETE", "TRUNCATED"].contains(&string(field(memory, "disposition")?)?)
                        && field(memory, "scope")?.is_string(),
                    "requested memory capture missing",
                )?;
                if memory["disposition"] == "TRUNCATED" {
                    reasons.push(format!("execution {ordinal}: memory producer truncation"));
                }
                payload.append(json!({"kind":"memory_scope","executionOrdinal":ordinal,"scope":memory["scope"]}),request)?;
                let rows = array(field(memory, "rows")?)?;
                for row in rows {
                    validate_memory_row(row)?;
                }
                rows
            } else {
                &[]
            };
            // Interleave categories so a long register trace does not consume
            // the whole export budget before the first requested memory row.
            // Each category retains its producer order; this is not a merged timeline.
            for index in 0..registers.len().max(memory_rows.len()) {
                if let Some(row) = registers.get(index) {
                    payload.append(
                        json!({"kind":"registers","executionOrdinal":ordinal,"row":row}),
                        request,
                    )?;
                }
                if let Some(row) = memory_rows.get(index) {
                    payload.append(
                        json!({"kind":"memory","executionOrdinal":ordinal,"row":row}),
                        request,
                    )?;
                }
            }
        }
    }
    if payload.exhausted {
        reasons.push("aggregate export event/byte budget reached".into());
    }
    payload.bytes.push(b']');
    let artifact = json!({"schema":"svm-capture-artifact/v2","executionMode":request.execution_mode,"identitySha256":identity,
        "status":if reasons.is_empty(){"COMPLETE"}else if partial && !payload.exhausted{"PARTIAL"}else{"TRUNCATED"},
        "reason":if reasons.is_empty(){Value::Null}else{json!(reasons.join("; "))},
        "payload":{"sha256":Digest::of(&payload.bytes),"bytes":payload.bytes.len(),"events":payload.events},
        "coverage":{"calls":"entry-exit","accountData":"raw-bytes","computeUnits":"per-call-inclusive-exclusive","nativeBuiltins":"calls","sbpf":request.sbpf_observations}});
    validate_artifact(&artifact, request, identity, Some(&payload.bytes))?;
    Ok(CaptureExport {
        artifact,
        payload: payload.bytes,
    })
}
