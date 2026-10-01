use super::*;

pub type Result<T> = std::result::Result<T, SourceError>;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceError {
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
}

impl SourceError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }
    pub(in super::super) fn details(mut self, value: Value) -> Self {
        self.details = Some(value);
        self
    }
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.code, self.message)
    }
}

impl std::error::Error for SourceError {}

/// Implementations return owned identity and record snapshots, never current-state fallbacks.
pub trait HistoricalSource: Send + Sync {
    fn identity(&self) -> Value;
    fn inspect(&self, query: &Value) -> Result<Option<Value>>;
    /// Optional finalized-block discovery. Return a normal hash-pinned record,
    /// or None when this source cannot discover the requested historical slot.
    /// Discovery is provider evidence, not independent consensus verification.
    fn discover_block(&self, _slot: u64) -> Result<Option<Value>> {
        Ok(None)
    }
    /// Counters are instance-cumulative snapshots, never inferred from retained bytes.
    fn diagnostics(&self) -> Result<Value> {
        Ok(json!({"status":"NOT_REPORTED"}))
    }
}

pub(in super::super) fn local_diagnostics() -> Value {
    json!({"status":"LOCAL_ONLY","transport":{"requests":0,"account_reads":0,"downloaded_bytes":0},
        "meaning":"This built-in reads retained local data; provenance response sizes are not new downloads."})
}

pub(in super::super) fn require(ok: bool, code: &'static str, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(SourceError::new(code, message))
    }
}

pub(in super::super) fn object(
    value: &Value,
    fields: &[&str],
    code: &'static str,
    label: &str,
) -> Result<()> {
    require(
        value
            .as_object()
            .is_some_and(|o| o.len() == fields.len() && fields.iter().all(|f| o.contains_key(*f))),
        code,
        &format!("{label} fields"),
    )
}

pub(in super::super) fn json_value(value: &Value) -> Result<()> {
    validate_safe_numbers(value).map_err(|e| SourceError::new("SOURCE_INTEGRITY", e.to_string()))
}

pub(in super::super) fn text<'a>(
    value: &'a Value,
    code: &'static str,
    label: &str,
) -> Result<&'a str> {
    value.as_str().ok_or_else(|| SourceError::new(code, label))
}

pub(in super::super) fn hash(value: &Value, code: &'static str, label: &str) -> Result<Digest> {
    Digest::new(text(value, code, label)?).map_err(|_| SourceError::new(code, label))
}

pub(in super::super) fn slot(value: &Value, code: &'static str, label: &str) -> Result<u64> {
    value
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| SourceError::new(code, label))
}

pub(in super::super) fn address(
    value: &Value,
    size: usize,
    code: &'static str,
    label: &str,
) -> Result<()> {
    let value = text(value, code, label)?;
    require(value.len() <= if size == 32 { 44 } else { 88 }, code, label)?;
    let bytes = bs58::decode(value)
        .into_vec()
        .map_err(|_| SourceError::new(code, label))?;
    require(
        bytes.len() == size && bs58::encode(bytes).into_string() == value,
        code,
        label,
    )
}

pub(in super::super) fn public_id(value: &Value, code: &'static str, label: &str) -> Result<()> {
    let value = text(value, code, label)?;
    require(
        !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._:-".contains(&c)),
        code,
        label,
    )
}
