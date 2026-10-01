use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointMode {
    Create,
    Run,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub prefix_transactions_executed: u64,
    pub prefix_simulation_calls: u64,
    pub prefix_commit_calls: u64,
    pub prefix_transactions_reused: u64,
}

impl Metrics {
    pub fn add(&mut self, other: &Self) -> Result<(), Error> {
        fn sum(left: u64, right: u64) -> Result<u64, Error> {
            left.checked_add(right)
                .filter(|value| *value <= 9_007_199_254_740_991)
                .ok_or_else(|| Error::new("CACHE_METRICS", "prefix counter overflow"))
        }
        self.prefix_transactions_executed = sum(
            self.prefix_transactions_executed,
            other.prefix_transactions_executed,
        )?;
        self.prefix_simulation_calls =
            sum(self.prefix_simulation_calls, other.prefix_simulation_calls)?;
        self.prefix_commit_calls = sum(self.prefix_commit_calls, other.prefix_commit_calls)?;
        self.prefix_transactions_reused = sum(
            self.prefix_transactions_reused,
            other.prefix_transactions_reused,
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CheckpointResponse {
    pub checkpoint_sha256: Digest,
    pub metrics: Metrics,
    pub output: Option<Value>,
    pub reads: Value,
}

pub fn checkpoint_response(
    value: &Value,
    mode: CheckpointMode,
    prefix_count: usize,
) -> Result<CheckpointResponse, Error> {
    if value.get("schema").and_then(Value::as_str) != Some("svm-m10-checkpoint-output/v1") {
        return Err(
            Error::new("CACHE_PROTOCOL", "checkpoint response schema differs")
                .with_details(value.clone()),
        );
    }
    let expected = match mode {
        CheckpointMode::Create => "CREATED",
        CheckpointMode::Run => "EXECUTED",
    };
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("CACHE_PROTOCOL", "checkpoint response has no status"))?;
    if status != expected {
        return Err(
            Error::new(status, "checkpoint attempt is not complete").with_details(value.clone())
        );
    }
    let metrics: Metrics = serde_json::from_value(
        value
            .get("metrics")
            .ok_or_else(|| Error::new("CACHE_PROTOCOL", "missing checkpoint counters"))?
            .clone(),
    )
    .map_err(|error| Error::new("CACHE_PROTOCOL", format!("checkpoint counters: {error}")))?;
    for counter in [
        metrics.prefix_transactions_executed,
        metrics.prefix_simulation_calls,
        metrics.prefix_commit_calls,
        metrics.prefix_transactions_reused,
    ] {
        if counter > 9_007_199_254_740_991 {
            return Err(Error::new(
                "CACHE_PROTOCOL",
                "counter exceeds exact integer range",
            ));
        }
    }
    match mode {
        CheckpointMode::Run => {
            if metrics.prefix_transactions_executed != 0
                || metrics.prefix_simulation_calls != 0
                || metrics.prefix_commit_calls != 0
            {
                return Err(Error::new(
                    "RESTORE_REPLAYED_PREFIX",
                    "restored execution replayed predecessors",
                )
                .with_details(value.clone()));
            }
            if metrics.prefix_transactions_reused != prefix_count as u64 {
                return Err(Error::new(
                    "CHECKPOINT_IDENTITY",
                    "restored prefix count differs",
                ));
            }
        }
        CheckpointMode::Create => {
            if metrics.prefix_transactions_executed != prefix_count as u64
                || metrics.prefix_simulation_calls != prefix_count as u64
                || metrics.prefix_commit_calls > prefix_count as u64
                || metrics.prefix_transactions_reused != 0
            {
                return Err(Error::new(
                    "CHECKPOINT_IDENTITY",
                    "created prefix counters differ from the prepared predecessor set",
                ));
            }
        }
    }
    let checkpoint_sha256 = Digest::new(
        value
            .get("checkpointSha256")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("CACHE_PROTOCOL", "missing checkpoint digest"))?
            .to_owned(),
    )?;
    let output = value.get("output").cloned();
    if mode == CheckpointMode::Run && output.is_none() {
        return Err(Error::new("CACHE_PROTOCOL", "restored output missing"));
    }
    let reads = value
        .get("reads")
        .filter(|value| value.is_array())
        .ok_or_else(|| Error::new("CACHE_PROTOCOL", "guarded checkpoint reads missing"))?
        .clone();
    Ok(CheckpointResponse {
        checkpoint_sha256,
        metrics,
        output,
        reads,
    })
}

pub fn validate_checkpoint(
    bytes: &[u8],
    fixture: &Value,
    worker_hash: &Digest,
) -> Result<Digest, Error> {
    let value = svm_replay_protocol::parse_json(bytes)?;
    if value.get("schema").and_then(Value::as_str) != Some("svm-m10-boundary/v1")
        || value.get("fixtureSha256").and_then(Value::as_str)
            != Some(super::super::super::identity::checkpoint_fixture_hash(fixture)?.as_str())
        || value.get("workerSha256").and_then(Value::as_str) != Some(worker_hash.as_str())
    {
        return Err(Error::new(
            "CHECKPOINT_INCOMPATIBLE",
            "cached checkpoint schema/fixture/worker binding differs",
        ));
    }
    Ok(Digest::of(bytes))
}
