use std::{fmt, str::FromStr};
use svm_replay_protocol::Error;

/// A finalized historical boundary, not a request to execute a transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum After {
    Transaction(String),
    /// Initialize the next produced slot, before its first transaction.
    Slot(u64),
}

impl FromStr for After {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Error> {
        let invalid = || {
            Error::new(
                "INVALID_SELECTOR",
                "use a base58 transaction signature or a decimal slot number",
            )
        };
        let slot = match value.strip_prefix("slot:") {
            Some(slot) => Some(slot),
            None if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
                Some(value)
            }
            None => None,
        };
        if let Some(slot) = slot {
            if slot.is_empty()
                || !slot.bytes().all(|byte| byte.is_ascii_digit())
                || (slot.len() > 1 && slot.starts_with('0'))
            {
                return Err(invalid());
            }
            let slot: u64 = slot.parse().map_err(|_| invalid())?;
            if slot > 9_007_199_254_740_991 {
                return Err(invalid());
            }
            return Ok(Self::Slot(slot));
        }
        let signature = match value.strip_prefix("tx:") {
            Some(signature) => signature,
            None => value,
        };
        crate::shared::history::address(signature, 64).map_err(|_| invalid())?;
        Ok(Self::Transaction(signature.into()))
    }
}

impl fmt::Display for After {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transaction(signature) => write!(f, "tx:{signature}"),
            Self::Slot(slot) => write!(f, "slot:{slot}"),
        }
    }
}
