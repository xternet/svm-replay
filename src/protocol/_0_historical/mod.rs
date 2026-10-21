use crate::{parse_json, validate_safe_numbers, Digest, Error, Limits, MetadataPolicy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
mod _0_request;
use _0_request as request;

pub use request::HistoricalRequest;
mod _1_what_if;
use _1_what_if as what_if;

pub use what_if::{StateOverride, WhatIf, WhatIfMode, MAX_BUNDLE, MAX_STATE_OVERRIDES};
