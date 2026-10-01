use crate::Error;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest as _, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Digest(String);

impl Digest {
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(Error::new(
                "INVALID_DIGEST",
                "expected 64 lowercase hexadecimal characters",
            ));
        }
        Ok(Self(value))
    }
    pub fn of(bytes: impl AsRef<[u8]>) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes.as_ref())))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
