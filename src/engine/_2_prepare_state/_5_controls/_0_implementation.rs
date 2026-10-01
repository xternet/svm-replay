use super::*;

pub(in super::super) fn failure(message: impl Into<String>) -> Error {
    Error::new("PREPARED_BOUNDARY", message)
}

pub(in super::super) fn uint(value: Option<&Value>, name: &str) -> Result<u64, Error> {
    value
        .and_then(Value::as_u64)
        .filter(|v| *v <= 9_007_199_254_740_991)
        .ok_or_else(|| failure(format!("missing/invalid {name}")))
}

pub fn validate_boundary(request: &PreparedRequest, block: &Value) -> Result<(), Error> {
    let slot = uint(request.candidate.get("slot"), "candidate slot")?;
    let index = uint(request.candidate.get("transactionIndex"), "candidate index")?;
    let result = block
        .get("result")
        .ok_or_else(|| failure("missing full archived block"))?;
    if block.get("error").is_some() {
        return Err(failure("RPC error is not an archived block"));
    }
    let parent = uint(result.get("parentSlot"), "parent slot")?;
    if parent >= slot {
        return Err(failure("parent must precede target slot"));
    }
    let transactions = result
        .get("transactions")
        .and_then(Value::as_array)
        .ok_or_else(|| failure("complete block transactions required"))?;
    let archived = transactions
        .get(index as usize)
        .ok_or_else(|| failure("target index outside block"))?;
    let target = request
        .fixture
        .get("target")
        .and_then(Value::as_object)
        .ok_or_else(|| failure("missing fixture target"))?;
    for (name, expected) in [
        ("targetSlot", slot),
        ("parentSlot", parent),
        ("index", index),
    ] {
        if uint(target.get(name), name)? != expected {
            return Err(failure(format!("{name} differs from historical boundary")));
        }
    }
    let original = target
        .get("transactionBase64")
        .and_then(Value::as_str)
        .ok_or_else(|| failure("missing original wire bytes"))?;
    transaction::assert_historical(original, archived)?;
    let indices = target
        .get("prefixIndices")
        .and_then(Value::as_array)
        .ok_or_else(|| failure("missing predecessor indices"))?;
    let encoded = target
        .get("prefixTransactionsBase64")
        .and_then(Value::as_array)
        .ok_or_else(|| failure("missing predecessor bytes"))?;
    if indices.len() != encoded.len() {
        return Err(failure("prefix index/wire count differs"));
    }
    let mut previous = None;
    for (position, value) in indices.iter().enumerate() {
        let current = uint(Some(value), "prefix index")?;
        if current >= index || previous.is_some_and(|prior| prior >= current) {
            return Err(failure("prefix must be unique, ordered and before target"));
        }
        let transaction = transactions
            .get(current as usize)
            .ok_or_else(|| failure("prefix outside block"))?;
        let bytes = encoded[position]
            .as_str()
            .ok_or_else(|| failure("prefix wire must be base64 string"))?;
        transaction::assert_historical(bytes, transaction)?;
        previous = Some(current);
    }
    let replacement = target
        .get("replacementTransactionBase64")
        .ok_or_else(|| failure("missing replacement field"))?;
    if !replacement.is_null() {
        transaction::decode(
            replacement
                .as_str()
                .ok_or_else(|| failure("replacement must be string or null"))?,
        )?;
    }
    context::assert_bound_context(&request.fixture)?;
    crate::shared::dependencies::_8_credits::validate_preloads(&request.fixture, transactions)?;
    Ok(())
}

pub fn original_control(fixture: &Value) -> Result<Value, Error> {
    let mut original = fixture.clone();
    let object = original
        .as_object_mut()
        .ok_or_else(|| failure("fixture must be object"))?;
    let target = object
        .get_mut("target")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| failure("missing target"))?;
    target.insert("replacementTransactionBase64".into(), Value::Null);
    object.insert("accountOverride".into(), Value::Null);
    object.remove("requestedAccountOverrides");
    Ok(original)
}

pub fn has_variant(fixture: &Value) -> Result<bool, Error> {
    let target = fixture
        .get("target")
        .ok_or_else(|| failure("missing target"))?;
    let replacement = target
        .get("replacementTransactionBase64")
        .ok_or_else(|| failure("missing replacement"))?;
    let account = fixture
        .get("accountOverride")
        .ok_or_else(|| failure("missing account override field"))?;
    Ok(*replacement != json!(null)
        || !account.is_null()
        || fixture.get("requestedAccountOverrides").is_some())
}
