use super::*;

pub(in super::super) const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MetadataPolicy {
    #[serde(rename = "STRICT")]
    Strict,
    #[serde(rename = "ARCHIVED_COMPUTE_METER_WARNING")]
    ArchivedComputeMeterWarning,
}

impl std::str::FromStr for MetadataPolicy {
    type Err = VerificationError;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "STRICT" => Ok(Self::Strict),
            "ARCHIVED_COMPUTE_METER_WARNING" => Ok(Self::ArchivedComputeMeterWarning),
            _ => Err(VerificationError("unknown metadata policy".into())),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataDivergence {
    pub label: String,
    pub canonical_compute_units: u64,
    pub replay_compute_units: u64,
    pub compute_unit_delta: i64,
    pub canonical_log_count: usize,
    pub replay_log_count: usize,
    pub different_log_indices: Vec<usize>,
    pub meter_only_log_differences: bool,
}

pub(in super::super::super) fn complete_logs(logs: &[String]) -> Result<()> {
    check(
        !logs.iter().any(|line| line == "Log truncated"),
        "logs truncated; complete evidence unavailable",
    )
}

pub(in super::super) fn meter(line: &str) -> Result<(&str, i64, i64)> {
    // Exactly the runtime line, excluding colon-prefixed program text and newlines.
    let pieces: Vec<_> = line.split(' ').collect();
    check(
        pieces.len() == 8
            && pieces[0] == "Program"
            && pieces[2] == "consumed"
            && pieces[4] == "of"
            && pieces[6] == "compute"
            && pieces[7] == "units",
        "non-meter log mismatch",
    )?;
    check(
        !pieces[1].is_empty() && !pieces[1].chars().any(|c| c == ':' || c.is_whitespace()),
        "non-meter log mismatch",
    )?;
    let amount = |value: &str| -> Result<i64> {
        check(
            !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit()),
            "malformed meter log",
        )?;
        let parsed = value
            .parse::<u64>()
            .map_err(|error| VerificationError(format!("malformed meter log: {error}")))?;
        check(
            parsed <= MAX_SAFE_INTEGER,
            "malformed meter log: unsafe integer",
        )?;
        Ok(parsed as i64)
    };
    Ok((pieces[1], amount(pieces[3])?, amount(pieces[5])?))
}

pub fn compare_archived_compute_metadata(
    label: &str,
    canonical_logs: &[String],
    replay_logs: &[String],
    canonical_compute_units: u64,
    replay_compute_units: u64,
) -> Result<Option<MetadataDivergence>> {
    complete_logs(canonical_logs)?;
    complete_logs(replay_logs)?;
    check(
        canonical_compute_units <= MAX_SAFE_INTEGER && replay_compute_units <= MAX_SAFE_INTEGER,
        "compute units exceed safe integer range",
    )?;
    if canonical_compute_units == replay_compute_units && canonical_logs == replay_logs {
        return Ok(None);
    }
    let delta = canonical_compute_units as i64 - replay_compute_units as i64;
    check(
        canonical_logs.len() == replay_logs.len() && delta != 0,
        &format!("{label} unexplained archived execution metadata mismatch"),
    )?;
    let mut indices = Vec::new();
    let mut cumulative_delta = 0;
    for (index, (canonical, replay)) in canonical_logs.iter().zip(replay_logs).enumerate() {
        if canonical == replay {
            check(
                cumulative_delta == 0 || meter(canonical).is_err(),
                &format!("{label} meter difference disappears at {index}"),
            )?;
            continue;
        }
        let (program, consumed, available) = meter(canonical)?;
        let (replay_program, replay_consumed, replay_available) = meter(replay)?;
        check(program == replay_program, "non-meter program mismatch")?;
        let consumed_delta = consumed - replay_consumed;
        let available_delta = available - replay_available;
        // Remaining budget at call exit accounts for both inherited and local
        // cost differences. Sequential calls may explain the total gradually;
        // exit differences must progress monotonically to the declared total.
        let exit_delta = consumed_delta - available_delta;
        if !((cumulative_delta..=delta.abs()).contains(&(exit_delta * delta.signum()))
            && (delta.min(0)..=delta.max(0)).contains(&consumed_delta)
            && ((-delta).min(0)..=(-delta).max(0)).contains(&available_delta))
        {
            let differences: Vec<_> = canonical_logs
                .iter()
                .zip(replay_logs)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .collect();
            return Err(VerificationError(format!(
                "{label} unexplained meter log mismatch at {index}: archived={canonical:?}, replay={replay:?}, total delta {delta}; changed lines={differences:?}"
            )));
        }
        cumulative_delta = exit_delta * delta.signum();
        indices.push(index);
    }
    check(
        indices.is_empty() || cumulative_delta == delta.abs(),
        &format!("{label} meter changes do not reconcile with total delta {delta}"),
    )?;
    Ok(Some(MetadataDivergence {
        label: label.into(),
        canonical_compute_units,
        replay_compute_units,
        compute_unit_delta: delta,
        canonical_log_count: canonical_logs.len(),
        replay_log_count: replay_logs.len(),
        different_log_indices: indices,
        meter_only_log_differences: true,
    }))
}
