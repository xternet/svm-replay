//! Exact JSON comparison helpers. Never route lamports or token units through f64.
use serde_json::Value;

pub fn exact_u64(value: &Value) -> Result<u64, String> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| "expected exact unsigned integer".into()),
        Value::String(value)
            if !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value == "0" || !value.starts_with('0')) =>
        {
            value
                .parse::<u64>()
                .map_err(|error| format!("unsigned integer out of u64 range: {error}"))
        }
        _ => Err("expected canonical unsigned integer".into()),
    }
}

pub fn canonical_json(value: &Value) -> String {
    match value {
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => {
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort();
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|key| format!(
                        "{}:{}",
                        Value::String(key.clone()),
                        canonical_json(&values[key])
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        _ => value.to_string(),
    }
}
