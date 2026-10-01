use super::*;

pub const MAX_BLOB_BYTES: u64 = 256 * 1024 * 1024;

pub const MAX_REQUEST_BYTES: u64 = 360 * 1024 * 1024;

pub(in super::super) const MAX_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug)]
pub struct StoreError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    pub details: Option<Value>,
}

pub type Result<T> = std::result::Result<T, StoreError>;

impl StoreError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
            details: None,
        }
    }
    pub fn response(&self) -> Value {
        let mut response = json!({"version":1,"status":"ERROR","error":{"code":self.code,"message":self.message,"retryable":self.retryable}});
        if let Some(details) = &self.details {
            response["details"] = details.clone();
        }
        response
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        Self::new("IO_ERROR", e.to_string())
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        let busy = matches!(&e, rusqlite::Error::SqliteFailure(x, _) if matches!(x.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked));
        Self {
            code: if busy { "STORE_BUSY" } else { "DATABASE_ERROR" },
            message: e.to_string(),
            retryable: busy,
            details: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(in super::super) enum Namespace {
    Raw,
    Prepared,
    Result,
}

impl Namespace {
    pub(in super::super) fn as_str(&self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Prepared => "prepared",
            Self::Result => "result",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in super::super) struct Lease {
    pub(in super::super) owner: String,
    pub(in super::super) expires_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Counts {
    pub(in super::super) reads: u64,
    pub(in super::super) requests: u64,
    pub(in super::super) bytes: u64,
}

impl Counts {
    pub(in super::super) fn validate(self) -> Result<Self> {
        integer(self.reads)?;
        integer(self.requests)?;
        integer(self.bytes)?;
        Ok(self)
    }
    pub(in super::super) fn value(self) -> Value {
        json!({"reads":self.reads,"requests":self.requests,"bytes":self.bytes})
    }
    pub(in super::super) fn add(self, other: Self) -> Result<Self> {
        let add = |a: u64, b: u64| {
            a.checked_add(b)
                .filter(|v| *v <= MAX_INTEGER)
                .ok_or_else(|| {
                    StoreError::new(
                        "BUDGET_OVERFLOW",
                        "cumulative counters exceed exact supported integer range",
                    )
                })
        };
        Ok(Self {
            reads: add(self.reads, other.reads)?,
            requests: add(self.requests, other.requests)?,
            bytes: add(self.bytes, other.bytes)?,
        })
    }
    pub(in super::super) fn exceeds(self, limit: Self) -> bool {
        self.reads > limit.reads || self.requests > limit.requests || self.bytes > limit.bytes
    }
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub(in super::super) enum Command {
    Put {
        version: u64,
        namespace: Namespace,
        key: String,
        #[serde(rename = "dataBase64")]
        data_base64: String,
        lease: Option<Lease>,
    },
    Get {
        version: u64,
        namespace: Namespace,
        key: String,
        lease: Option<Lease>,
    },
    Pin {
        version: u64,
        namespace: Namespace,
        key: String,
        owner: String,
    },
    Unpin {
        version: u64,
        namespace: Namespace,
        key: String,
        owner: String,
    },
    Lease {
        version: u64,
        namespace: Namespace,
        key: String,
        owner: String,
        #[serde(rename = "expiresAtMs")]
        expires_at_ms: u64,
    },
    Release {
        version: u64,
        namespace: Namespace,
        key: String,
        owner: String,
    },
    Evict {
        version: u64,
        #[serde(rename = "maxBytes")]
        max_bytes: u64,
    },
    Budget {
        version: u64,
        action: String,
        id: String,
        limits: Option<Counts>,
        amount: Option<Counts>,
    },
}
