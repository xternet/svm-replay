use super::*;

pub(in super::super::super) fn check(condition: bool, message: &str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::new("CAPTURE_FORMAT", message))
    }
}

pub(in super::super::super) fn shape(value: &Value, fields: &[&str]) -> Result<(), Error> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("CAPTURE_FORMAT", "expected object"))?;
    check(
        object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field)),
        "missing/unexpected fields",
    )
}

pub(in super::super::super) fn key(value: &str) -> Result<(), Error> {
    let bytes = bs58::decode(value)
        .into_vec()
        .map_err(|error| Error::new("CAPTURE_FORMAT", format!("invalid key: {error}")))?;
    check(
        bytes.len() == 32 && bs58::encode(bytes).into_string() == value,
        "key is not canonical 32-byte base58",
    )
}

pub(in super::super) fn member(value: &str, choices: &[&str]) -> Result<(), Error> {
    check(choices.contains(&value), "unknown capture declaration")
}

pub(in super::super) const SAFE: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureRequest {
    pub schema: String,
    pub execution_mode: String,
    pub level: String,
    pub sbpf_observations: String,
    pub filter: CaptureFilter,
    pub limits: CaptureLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureFilter {
    pub program_ids: Vec<String>,
    pub instruction_indices: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureLimits {
    pub max_bytes: u64,
    pub max_events: u64,
    pub timeout_ms: u64,
}

impl CaptureRequest {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let value: Self = serde_json::from_value(parse_json(bytes)?)
            .map_err(|error| Error::new("CAPTURE_FORMAT", error.to_string()))?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), Error> {
        check(
            self.schema == "svm-capture-request/v2",
            "unknown capture request schema",
        )?;
        member(
            &self.execution_mode,
            &["jit", "interpreter", "interpreter-debug"],
        )?;
        member(&self.level, &["off", "calls", "sbpf"])?;
        member(
            &self.sbpf_observations,
            &["none", "pc-registers", "pc-registers-memory"],
        )?;
        check(
            (self.level == "sbpf") == (self.sbpf_observations != "none"),
            "capture level/SBPF observations differ",
        )?;
        for limit in [
            self.limits.max_bytes,
            self.limits.max_events,
            self.limits.timeout_ms,
        ] {
            check(
                limit > 0 && limit <= SAFE,
                "capture budget is not a positive exact integer",
            )?;
        }
        for id in &self.filter.program_ids {
            key(id)?;
        }
        check(
            self.filter
                .program_ids
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                == self.filter.program_ids.len()
                && self
                    .filter
                    .instruction_indices
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    == self.filter.instruction_indices.len()
                && self
                    .filter
                    .instruction_indices
                    .iter()
                    .all(|index| *index <= SAFE),
            "invalid/duplicate capture filter",
        )?;
        check(
            self.level != "off"
                || self.filter.program_ids.is_empty() && self.filter.instruction_indices.is_empty(),
            "off capture has active filters",
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProducerBounds {
    pub register_rows: u64,
    pub memory_rows: Option<u64>,
    pub max_invocations: u64,
}

impl ProducerBounds {
    pub fn validate(&self, request: &CaptureRequest) -> Result<(), Error> {
        request.validate()?;
        check(
            request.limits.max_bytes <= 256 * 1024 * 1024
                && request.limits.max_events <= 1_000_000
                && request.limits.timeout_ms <= 900_000,
            "request exceeds bounded local capture adapter",
        )?;
        check(
            self.register_rows <= 2_000_000
                && self.memory_rows.is_none_or(|rows| rows <= 2_000_000)
                && self.max_invocations > 0
                && self.max_invocations <= 1024,
            "invalid producer bounds",
        )?;
        check(
            (request.sbpf_observations == "pc-registers-memory") == self.memory_rows.is_some(),
            "memory policy/producer bound differs",
        )?;
        check(
            request.level == "sbpf" || self.register_rows == 0,
            "register capture outside SBPF request",
        )
    }
}
