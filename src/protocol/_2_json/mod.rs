use crate::Error;
use serde::{
    de::{MapAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{value::RawValue, Map, Value};
use std::collections::BTreeMap;
mod _0_parse;
use _0_parse as parse;

pub use parse::{parse_json, validate_safe_numbers};
