use super::*;

pub fn validate_artifact(
    value: &Value,
    request: &CaptureRequest,
    identity: &Digest,
    payload: Option<&[u8]>,
) -> Result<(), Error> {
    request.validate()?;
    shape(
        value,
        &[
            "schema",
            "executionMode",
            "identitySha256",
            "status",
            "reason",
            "payload",
            "coverage",
        ],
    )?;
    check(
        value["schema"] == "svm-capture-artifact/v2",
        "unknown artifact schema",
    )?;
    if value["executionMode"] != request.execution_mode
        || value["identitySha256"] != json!(identity)
    {
        return Err(Error::new(
            "CAPTURE_IDENTITY",
            "artifact mode/request identity differs",
        ));
    }
    let status = value["status"]
        .as_str()
        .ok_or_else(|| Error::new("CAPTURE_FORMAT", "artifact status missing"))?;
    member(
        status,
        &[
            "OFF",
            "COMPLETE",
            "PARTIAL",
            "TRUNCATED",
            "UNAVAILABLE",
            "ERROR",
            "CANCELLED",
            "TIMEOUT",
        ],
    )?;
    check(
        if ["OFF", "COMPLETE"].contains(&status) {
            value["reason"].is_null()
        } else {
            value["reason"]
                .as_str()
                .is_some_and(|reason| !reason.trim().is_empty())
        },
        "capture disposition/reason differs",
    )?;
    let coverage = &value["coverage"];
    shape(
        coverage,
        &[
            "calls",
            "sbpf",
            "accountData",
            "computeUnits",
            "nativeBuiltins",
        ],
    )?;
    for (field, choices) in [
        ("calls", &["none", "recorded-invocations", "entry-exit"][..]),
        ("sbpf", &["none", "pc-registers", "pc-registers-memory"]),
        ("accountData", &["none", "hashes", "raw-bytes"]),
        ("computeUnits", &["none", "per-call-inclusive-exclusive"]),
        ("nativeBuiltins", &["none", "calls"]),
    ] {
        member(
            coverage[field]
                .as_str()
                .ok_or_else(|| Error::new("CAPTURE_FORMAT", "coverage declaration missing"))?,
            choices,
        )?;
    }
    check(
        (request.level == "off") == (status == "OFF"),
        "off request/status mismatch",
    )?;
    if value["payload"].is_null() {
        check(
            payload.is_none()
                && coverage
                    .as_object()
                    .expect("validated object")
                    .values()
                    .all(|value| value == "none")
                && !["COMPLETE", "PARTIAL", "TRUNCATED"].contains(&status),
            "absent payload claims observations",
        )?;
        return Ok(());
    }
    check(
        !["OFF", "UNAVAILABLE"].contains(&status),
        "off/unavailable carries payload",
    )?;
    shape(&value["payload"], &["sha256", "bytes", "events"])?;
    let bytes = payload.ok_or_else(|| Error::new("CAPTURE_FORMAT", "payload bytes absent"))?;
    let count = value["payload"]["events"]
        .as_u64()
        .ok_or_else(|| Error::new("CAPTURE_FORMAT", "invalid event count"))?;
    if value["payload"]["bytes"] != json!(bytes.len())
        || bytes.len() as u64 > request.limits.max_bytes
        || count > request.limits.max_events
    {
        return Err(Error::new(
            "CAPTURE_LIMIT",
            "payload byte/event bound differs",
        ));
    }
    if value["payload"]["sha256"] != json!(Digest::of(bytes)) {
        return Err(Error::new("CAPTURE_INTEGRITY", "payload digest differs"));
    }
    let events = parse_json(bytes)?;
    check(
        events.as_array().is_some_and(|events| {
            events.len() as u64 == count && events.iter().all(Value::is_object)
        }),
        "invalid event array/count",
    )?;
    if status == "COMPLETE" {
        check(
            coverage["calls"] == "entry-exit"
                && coverage["accountData"] == "raw-bytes"
                && coverage["computeUnits"] == "per-call-inclusive-exclusive"
                && coverage["nativeBuiltins"] == "calls"
                && coverage["sbpf"] == request.sbpf_observations,
            "complete coverage lacks requested observations",
        )?;
    }
    Ok(())
}

/// A digest is never substituted for an actual raw account-data snapshot.
pub fn validate_raw_account_data(value: &Value) -> Result<(), Error> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    shape(value, &["dataBase64", "dataSha256"])?;
    let encoded = value["dataBase64"]
        .as_str()
        .ok_or_else(|| Error::new("CAPTURE_FORMAT", "raw account bytes missing"))?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| Error::new("CAPTURE_INTEGRITY", error.to_string()))?;
    if STANDARD.encode(&bytes) != encoded || value["dataSha256"] != json!(Digest::of(&bytes)) {
        return Err(Error::new(
            "CAPTURE_INTEGRITY",
            "raw account bytes/digest differ",
        ));
    }
    Ok(())
}
