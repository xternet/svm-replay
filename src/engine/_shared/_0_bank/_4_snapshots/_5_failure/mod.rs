use super::*;

pub(super) fn epoch_failure(value: &Value, fixture: &Value) -> Check<()> {
    shape(
        value,
        &[
            "schema",
            "status",
            "code",
            "reason",
            "guardedExecutionAttempted",
            "certifiedResult",
            "targetSlot",
            "epoch",
            "read",
        ],
    )?;
    require(
        field(value, "schema")? == "svm-epoch-stake-input-failure/v1"
            && field(value, "status")? == "UNSUPPORTED"
            && field(value, "code")? == "UNSUPPORTED_HISTORICAL_EPOCH_STAKE"
            && field(value, "reason")? == "complete current-epoch snapshot required"
            && field(value, "guardedExecutionAttempted")? == &json!(true)
            && field(value, "certifiedResult")? == &json!(false),
        "epoch failure schema/status mismatch",
    )?;
    let target_slot = slot(field(value, "targetSlot")?)?;
    let epoch = u64_string(field(value, "epoch")?)?;
    let read = field(value, "read")?;
    match string(field(read, "kind")?)? {
        "total" => shape(read, &["kind"])?,
        "vote" => {
            shape(read, &["kind", "voteAccount"])?;
            pubkey(field(read, "voteAccount")?)?;
        }
        _ => return Err("unknown epoch failure read kind".into()),
    }
    let fixture_slot = slot(field(field(fixture, "target")?, "targetSlot")?)?;
    let clock = clock_data(field(fixture, "clock")?, fixture_slot)?;
    let clock_epoch = u64::from_le_bytes(
        clock[16..24]
            .try_into()
            .map_err(|error| format!("Clock epoch: {error}"))?,
    );
    require(
        target_slot == fixture_slot
            && epoch == clock_epoch
            && !object(field(fixture, "runtime")?)?.contains_key("epochStakes"),
        "epoch failure context mismatch",
    )
}

/// Validate an explicit worker missing-cache veto before classifying it as unsupported.
pub fn validate_epoch_stake_failure(value: &Value, fixture: &Value) -> Result<(), Error> {
    epoch_failure(value, fixture).map_err(|error| Error::new("INVALID_EPOCH_STAKE_INPUT", error))
}
