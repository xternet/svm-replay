use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};

/// Most transactions one request may run after or instead of the target.
pub const MAX_BUNDLE: usize = 16;
/// Most accounts one request may override.
pub const MAX_STATE_OVERRIDES: usize = 64;
const MAX_ACCOUNT_DATA: usize = 10 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WhatIfMode {
    /// The transactions run right after the target, on the state it left.
    After,
    /// The transactions run where the target would have run; the target does not.
    Replace,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WhatIf {
    pub mode: WhatIfMode,
    /// Omitted: the target itself runs again (mode `replace`), on the overridden state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transactions_base64: Option<Vec<String>>,
    /// Applied just before the first transaction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_overrides: Vec<StateOverride>,
}

/// With `owner`, the override replaces the whole account; left-out fields are: not executable,
/// no data, rent epoch u64::MAX and the rent-exempt minimum of lamports. Without `owner`, the
/// given fields patch the existing account. `lamports: "0"` deletes the account.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateOverride {
    pub pubkey: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lamports: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rent_epoch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
}

impl WhatIf {
    pub fn validate(&self) -> Result<(), Error> {
        let invalid = |message: String| Error::new("INVALID_REQUEST", message);
        match &self.transactions_base64 {
            None if self.mode == WhatIfMode::After => {
                return Err(invalid("whatIf mode after needs transactionsBase64".into()))
            }
            None if self.state_overrides.is_empty() => {
                return Err(invalid(
                    "whatIf needs transactionsBase64 or stateOverrides".into(),
                ))
            }
            None => {}
            Some(list) if list.is_empty() || list.len() > MAX_BUNDLE => {
                return Err(invalid(format!(
                    "a bundle has 1 to {MAX_BUNDLE} transactions"
                )))
            }
            Some(list) => {
                for wire in list {
                    crate::transaction::decode(wire)?;
                }
            }
        }
        if self.state_overrides.len() > MAX_STATE_OVERRIDES {
            return Err(invalid(format!(
                "at most {MAX_STATE_OVERRIDES} state overrides"
            )));
        }
        let mut seen = std::collections::BTreeSet::new();
        for entry in &self.state_overrides {
            entry.validate()?;
            if !seen.insert(entry.pubkey.as_str()) {
                return Err(entry.invalid("one override per account"));
            }
        }
        Ok(())
    }
}

impl StateOverride {
    fn invalid(&self, message: &str) -> Error {
        Error::new(
            "INVALID_STATE_OVERRIDE",
            format!("{}: {message}", self.pubkey),
        )
    }

    pub fn validate(&self) -> Result<(), Error> {
        if !is_address(&self.pubkey) {
            return Err(self.invalid("pubkey must be a base58 address"));
        }
        if self.lamports.is_none()
            && self.owner.is_none()
            && self.executable.is_none()
            && self.rent_epoch.is_none()
            && self.data_base64.is_none()
        {
            return Err(self.invalid("set at least one field"));
        }
        for (name, value) in [
            ("lamports", &self.lamports),
            ("rentEpoch", &self.rent_epoch),
        ] {
            if value.as_deref().is_some_and(|text| !is_u64(text)) {
                return Err(self.invalid(&format!("{name} must be a u64 decimal string")));
            }
        }
        if self
            .owner
            .as_deref()
            .is_some_and(|owner| !is_address(owner))
        {
            return Err(self.invalid("owner must be a base58 address"));
        }
        if let Some(encoded) = &self.data_base64 {
            let canonical = encoded.len() <= MAX_ACCOUNT_DATA.div_ceil(3) * 4
                && STANDARD.decode(encoded).is_ok_and(|data| {
                    data.len() <= MAX_ACCOUNT_DATA && STANDARD.encode(data) == *encoded
                });
            if !canonical {
                return Err(self.invalid("dataBase64 must be padded base64 of at most 10 MiB"));
            }
        }
        Ok(())
    }
}

fn is_address(text: &str) -> bool {
    bs58::decode(text)
        .into_vec()
        .is_ok_and(|bytes| bytes.len() == 32 && bs58::encode(bytes).into_string() == text)
}

fn is_u64(text: &str) -> bool {
    !text.is_empty()
        && text.bytes().all(|b| b.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'))
        && text.parse::<u64>().is_ok()
}
