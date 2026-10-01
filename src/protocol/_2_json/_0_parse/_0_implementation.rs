use super::*;

// Preserve raw child tokens before recursively constructing Values: serde_json's
// default object decoder otherwise silently overwrites duplicate keys.
pub(in super::super) struct UniqueObject(pub(in super::super) BTreeMap<String, Box<RawValue>>);

impl<'de> Deserialize<'de> for UniqueObject {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = UniqueObject;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("unique JSON object keys")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
                    if entries.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!("duplicate field: {key}")));
                    }
                }
                Ok(UniqueObject(entries))
            }
        }
        de.deserialize_map(ObjectVisitor)
    }
}

pub fn parse_json(bytes: &[u8]) -> Result<Value, Error> {
    let raw: Box<RawValue> = serde_json::from_slice(bytes).map_err(json_error)?;
    decode(raw.get(), 0)
}

pub(in super::super) fn json_error(error: serde_json::Error) -> Error {
    Error::new("INVALID_JSON", error.to_string())
}

pub(in super::super) fn decode(raw: &str, depth: usize) -> Result<Value, Error> {
    if depth > 128 {
        return Err(Error::new("INVALID_JSON", "nesting exceeds 128"));
    }
    match raw.as_bytes().first() {
        Some(b'{') => {
            let object: UniqueObject = serde_json::from_str(raw).map_err(json_error)?;
            let mut decoded = Map::new();
            for (key, value) in object.0 {
                decoded.insert(key, decode(value.get(), depth + 1)?);
            }
            Ok(Value::Object(decoded))
        }
        Some(b'[') => {
            let children: Vec<Box<RawValue>> = serde_json::from_str(raw).map_err(json_error)?;
            children
                .iter()
                .map(|v| decode(v.get(), depth + 1))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        _ => serde_json::from_str(raw).map_err(json_error),
    }
}

pub fn validate_safe_numbers(value: &Value) -> Result<(), Error> {
    match value {
        Value::Number(number) => {
            let token = number.to_string();
            let safe = token
                .parse::<i64>()
                .is_ok_and(|v| (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&v));
            if !safe {
                return Err(Error::new(
                    "UNSAFE_NUMBER",
                    "protocol numbers must be safe integers; encode u64 values as decimal strings",
                ));
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_safe_numbers(value)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                validate_safe_numbers(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
